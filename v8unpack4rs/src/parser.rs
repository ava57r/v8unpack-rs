use container::*;
use error;

use std::{cmp, fs, path, str};
use std::io::{self, BufReader, Cursor, Error as ioError, ErrorKind as ioErrorKind, SeekFrom};
use std::io::prelude::*;
use std::sync::mpsc::{sync_channel, Receiver};
use std::thread::{spawn, JoinHandle};

use inflate;

/// Contains methods for working with file format 1C: Enterprise 8 `1cd`.
pub struct Parser;

impl Parser {
    /// Makes the unpacking of the container to a directory on disk.
    pub fn unpack_to_directory_no_load(
        file_name: &str,
        dir_name: &str,
        bool_inflate: bool,
        _unpack_when_need: bool,
    ) -> Result<bool> {
        let file = fs::File::open(file_name)?;
        let mut buf_reader = BufReader::new(file);

        if !buf_reader.is_v8file() {
            return Ok(false);
        }

        let first_block_header = buf_reader.get_first_block_header()?;

        let p_dir = path::Path::new(dir_name);
        if !p_dir.exists() {
            fs::create_dir(dir_name)?;
        }

        let elems_addrs = Parser::read_elems_addrs(&mut buf_reader, &first_block_header)?;

        for cur_elem in elems_addrs.iter() {
            if cur_elem.fffffff != V8_MAGIC_NUMBER {
                break;
            }

            buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

            let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;

            if !elem_block_header.is_correct() {
                return Err(error::V8Error::NotV8File);
            }

            let elem_block_data = Parser::read_block_data(&mut buf_reader, &elem_block_header)?;
            let elem_name = V8Elem::new().with_header(elem_block_data).get_name()?;

            let elem_path = p_dir.join(&elem_name);

            if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                let _result = Parser::process_data(&mut buf_reader, bool_inflate, &elem_path)?;
            }
        }
        Ok(true)
    }

    fn start_inflate_thread(
        rawdata: Receiver<RawData>,
    ) -> (Receiver<(Vec<u8>, V8Elem)>, JoinHandle<Result<()>>) {
        let (sender, receiver) = sync_channel(128);

        let handle = spawn(move || {
            for item in rawdata {
                let (block_data, elem_block_data) = (item.block_data, item.elem_block_data);
                let out_data = match inflate::inflate_bytes(&block_data) {
                    Ok(inf_bytes) => inf_bytes,
                    Err(_) => block_data,
                };

                let elem = V8Elem::new().with_header(elem_block_data);

                if sender.send((out_data, elem)).is_err() {
                    break;
                }
            }
            Ok(())
        });

        (receiver, handle)
    }

    fn start_file_parse(
        data: Receiver<(Vec<u8>, V8Elem)>,
        p_dir: &path::Path,
        bool_inflate: bool,
    ) -> Result<bool> {
        for item in data {
            let elem_path = p_dir.join(&item.1.get_name()?);

            let mut rdr = Cursor::new(&item.0);
            if rdr.is_v8file() {
                Parser::load_file(&mut rdr, bool_inflate)?.save_file_to_folder(&elem_path)?;
            } else {
                fs::File::create(elem_path.as_path())?.write_all(&item.0)?;
            }
        }
        Ok(true)
    }

    pub fn parse_to_folder(file_name: &str, dir_name: &str, bool_inflate: bool) -> Result<bool> {
        let p_dir = path::Path::new(dir_name);
        if !p_dir.exists() {
            fs::create_dir(dir_name)?;
        };

        let (_, elems_addrs) = Parser::read_content(file_name)?;
        let (rawdata, h1) =
            Parser::start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);
        let (inf_data, h2) = Parser::start_inflate_thread(rawdata);

        let result = Parser::start_file_parse(inf_data, p_dir, bool_inflate);

        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();

        r1?;
        r2?;

        result
    }

    /// Parses the container into its component parts so that the elements
    /// are saved in binary format in a directory on disk.
    pub fn unpack_to_folder(file_name: &str, dir_name: &str) -> Result<bool> {
        let file = fs::File::open(file_name)?;
        let mut buf_reader = BufReader::new(file);

        if !buf_reader.is_v8file() {
            return Ok(false);
        }

        let p_dir = path::Path::new(dir_name);
        if !p_dir.exists() {
            fs::create_dir(dir_name)?;
        }

        let file_header = buf_reader.get_file_header()?.into_bytes()?;
        fs::File::create(p_dir.join("FileHeader"))?.write_all(&file_header)?;

        let first_block_header = buf_reader.get_first_block_header()?;

        let elems_addrs = Parser::read_elems_addrs(&mut buf_reader, &first_block_header)?;

        for cur_elem in elems_addrs.iter() {
            if cur_elem.fffffff != V8_MAGIC_NUMBER {
                break;
            }

            buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

            let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;

            if !elem_block_header.is_correct() {
                return Err(error::V8Error::NotV8File);
            }

            let elem_block_data = Parser::read_block_data(&mut buf_reader, &elem_block_header)?;
            let v8_elem = V8Elem::new().with_header(elem_block_data);
            let elem_name = v8_elem.get_name()?;

            let mut file_elem_header = String::new();
            file_elem_header.push_str(&elem_name);
            file_elem_header.push_str(".header");

            fs::File::create(p_dir.join(&file_elem_header))?.write_all(&v8_elem.get_header())?;

            if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                let block_header_data = BlockHeader::from_raw_parts(&mut buf_reader)?;

                let block_data = Parser::read_block_data(&mut buf_reader, &block_header_data)?;
                let mut file_elem_data = String::new();
                file_elem_data.push_str(&elem_name);
                file_elem_data.push_str(".data");
                fs::File::create(p_dir.join(&file_elem_data))?.write_all(&block_data)?;
            }
        }
        Ok(true)
    }

    fn start_file_reader_thread(
        file_name: path::PathBuf,
        elems_addrs: Vec<ElemAddr>,
    ) -> (Receiver<RawData>, JoinHandle<Result<()>>) {
        let (sender, receiver) = sync_channel(128);

        let handle = spawn(move || {
            let file = fs::File::open(file_name)?;
            let mut buf_reader = BufReader::new(file);

            for cur_elem in elems_addrs.iter() {
                if cur_elem.fffffff != V8_MAGIC_NUMBER {
                    break;
                }

                buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;
                let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;
                if !elem_block_header.is_correct() {
                    return Err(error::V8Error::NotV8File);
                }

                let elem_block_data = Parser::read_block_data(&mut buf_reader, &elem_block_header)?;

                let mut block_data = vec![];

                if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                    buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                    let block_header_data = BlockHeader::from_raw_parts(&mut buf_reader)?;

                    block_data = Parser::read_block_data(&mut buf_reader, &block_header_data)?;
                }

                if sender
                    .send(RawData {
                        elem_block_data,
                        block_data,
                    })
                    .is_err()
                {
                    break;
                }
            }
            Ok(())
        });

        (receiver, handle)
    }

    fn start_file_write(rawdata: Receiver<RawData>, p_dir: &path::Path) -> Result<bool> {
        for item in rawdata {
            let v8_elem = V8Elem::new().with_header(item.elem_block_data);
            let elem_name = v8_elem.get_name()?;

            let mut file_elem_header = String::new();
            file_elem_header.push_str(&elem_name);
            file_elem_header.push_str(".header");

            fs::File::create(p_dir.join(&file_elem_header))?.write_all(&v8_elem.get_header())?;

            let mut file_elem_data = String::new();
            file_elem_data.push_str(&elem_name);
            file_elem_data.push_str(".data");
            fs::File::create(p_dir.join(&file_elem_data))?.write_all(&item.block_data)?;
        }

        Ok(true)
    }

    pub fn unpack_pipeline(file_name: &str, dir_name: &str) -> Result<bool> {
        let p_dir = path::Path::new(dir_name);
        if !p_dir.exists() {
            fs::create_dir(dir_name)?;
        };

        let (file_header, elems_addrs) = Parser::read_content(file_name)?;
        fs::File::create(p_dir.join("FileHeader"))?.write_all(&file_header.into_bytes()?)?;

        let (rawdata, h1) =
            Parser::start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);

        let result = Parser::start_file_write(rawdata, p_dir);

        let r1 = h1.join().unwrap();

        r1?;

        result
    }

    fn read_content(file_name: &str) -> Result<(FileHeader, Vec<ElemAddr>)> {
        let file = fs::File::open(file_name)?;
        let mut buf_reader = BufReader::new(file);

        if !buf_reader.is_v8file() {
            return Err(error::V8Error::NotV8File);
        }

        let file_header = buf_reader.get_file_header()?;
        let first_block_header = buf_reader.get_first_block_header()?;
        let elems_addrs = Parser::read_elems_addrs(&mut buf_reader, &first_block_header)?;

        Ok((file_header, elems_addrs))
    }

    fn read_elems_addrs<R>(src: &mut R, block_header: &BlockHeader) -> Result<Vec<ElemAddr>>
    where
        R: Read + Seek,
    {
        let block_data = Parser::read_block_data(src, block_header)?;
        let data_size = block_data.len() as u64;
        let mut rdr = Cursor::new(block_data);

        let mut elems_addrs: Vec<ElemAddr> = vec![];

        while rdr.position() < data_size {
            elems_addrs.push(ElemAddr::from_raw_parts(&mut rdr)?);
        }

        Ok(elems_addrs)
    }

    pub fn read_block_data<R>(src: &mut R, block_header: &BlockHeader) -> Result<Vec<u8>>
    where
        R: Read + Seek,
    {
        let data_size = block_header.get_data_size()?;

        let mut result: Vec<u8> = Vec::with_capacity(data_size as usize);

        let mut read_in_bytes = 0;

        let mut local_block_header = block_header.clone();
        while read_in_bytes < data_size {
            let page_size = local_block_header.get_page_size()?;
            let next_page_addr = local_block_header.get_next_page_addr()?;

            let bytes_to_read = cmp::min(page_size, data_size - read_in_bytes);
            let mut lbuf: Vec<u8> = Vec::with_capacity(bytes_to_read as usize);
            let read_b = src.take(bytes_to_read as u64).read_to_end(&mut lbuf)?;

            read_in_bytes += bytes_to_read;
            if read_b < bytes_to_read as usize {
                return Err(error::V8Error::IoError(ioError::new(
                    ioErrorKind::InvalidData,
                    "Readied too few bytes",
                )));
            }

            result.extend(lbuf.iter());

            if next_page_addr != V8_MAGIC_NUMBER {
                src.seek(SeekFrom::Start(next_page_addr as u64))?;
                local_block_header = BlockHeader::from_raw_parts(src)?;
            } else {
                break;
            }
        }

        Ok(result)
    }

    pub fn process_data(
        src: &mut BufReader<fs::File>,
        _need_unpack: bool,
        elem_path: &path::PathBuf,
    ) -> Result<bool> {
        let header = BlockHeader::from_raw_parts(src)?;
        if !header.is_correct() {
            return Err(error::V8Error::NotV8File);
        }

        let block_data = Parser::read_block_data(src, &header)?;
        let out_data = match inflate::inflate_bytes(&block_data) {
            Ok(inf_bytes) => inf_bytes,
            Err(_) => block_data,
        };

        let mut rdr = Cursor::new(&out_data);

        if rdr.is_v8file() {
            Parser::load_file(&mut rdr, _need_unpack)?.save_file_to_folder(elem_path)?;
        } else {
            fs::File::create(elem_path.as_path())?.write_all(&out_data)?;
        }

        Ok(true)
    }

    pub fn load_file<R>(src: &mut R, bool_inflate: bool) -> Result<V8File>
    where
        R: Read + Seek + V8Container,
    {
        let file_header = src.get_file_header()?;
        let first_block_header = src.get_first_block_header()?;

        let elems_addrs = Parser::read_elems_addrs(src, &first_block_header)?;
        let mut elems: Vec<V8Elem> = vec![];

        for cur_elem in elems_addrs.iter() {
            if cur_elem.fffffff != V8_MAGIC_NUMBER {
                break;
            }

            src.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

            let elem_block_header = BlockHeader::from_raw_parts(src)?;

            if !elem_block_header.is_correct() {
                return Err(error::V8Error::NotV8File);
            }

            let elem_block_header_data = Parser::read_block_data(src, &elem_block_header)?;

            let elem_block_data: Vec<u8> = if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                src.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                let block_header_data = BlockHeader::from_raw_parts(src)?;

                Parser::read_block_data(src, &block_header_data)?
            } else {
                vec![]
            };

            let out_data = match inflate::inflate_bytes(&elem_block_data) {
                Ok(inf_bytes) => inf_bytes,
                Err(_) => elem_block_data,
            };

            let mut rdr = Cursor::new(out_data);
            let is_v8file = rdr.is_v8file();

            let unpacked_data = if is_v8file {
                Parser::load_file(&mut rdr, bool_inflate)?
            } else {
                V8File::new()
            };

            let out_data = rdr.into_inner();

            elems.push(
                V8Elem::new()
                    .with_header(elem_block_header_data)
                    .with_data(out_data)
                    .with_unpacked_data(unpacked_data)
                    .is_v8file(is_v8file),
            );
        }

        Ok(V8File::new()
            .with_header(file_header)
            .with_elems_addrs(elems_addrs)
            .with_elems(elems))
    }
}

struct RawData {
    elem_block_data: Vec<u8>,
    block_data: Vec<u8>,
}
