use container::*;
use error;

use std::{fs, path, str};
use std::io::{BufReader, Cursor, SeekFrom};
use std::io::prelude::*;
use std::sync::mpsc::{sync_channel, Receiver};
use std::thread::{spawn, JoinHandle};

use inflate;
use super::single;

fn start_inflate_thread(rawdata: Receiver<RawData>) -> (Receiver<RawData>, JoinHandle<Result<()>>) {
    let (sender, receiver) = sync_channel(128);

    let handle = spawn(move || {
        for item in rawdata {
            let (block_data, v8_elem) = (item.block_data, item.v8_elem);
            let out_data = match inflate::inflate_bytes(&block_data) {
                Ok(inf_bytes) => inf_bytes,
                Err(_) => block_data,
            };

            if sender
                .send(RawData {
                    v8_elem: v8_elem,
                    block_data: out_data,
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

fn start_file_parse(
    data: Receiver<RawData>,
    p_dir: &path::Path,
    bool_inflate: bool,
) -> Result<bool> {
    for item in data {
        let (out_data, v8_elem) = (item.block_data, item.v8_elem);
        let elem_path = p_dir.join(&v8_elem.get_name()?);

        let mut rdr = Cursor::new(&out_data);
        if rdr.is_v8file() {
            single::load_file(&mut rdr, bool_inflate)?.save_file_to_folder(&elem_path)?;
        } else {
            fs::File::create(elem_path.as_path())?.write_all(&out_data)?;
        }
    }
    Ok(true)
}

pub fn parse_to_folder(file_name: &str, dir_name: &str, bool_inflate: bool) -> Result<bool> {
    let p_dir = path::Path::new(dir_name);
    if !p_dir.exists() {
        fs::create_dir(dir_name)?;
    };

    let (_, elems_addrs) = read_content(file_name)?;
    let (rawdata, h1) = start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);
    let (inf_data, h2) = start_inflate_thread(rawdata);

    let result = start_file_parse(inf_data, p_dir, bool_inflate);

    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();

    r1?;
    r2?;

    result
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

            let elem_block_data = single::read_block_data(&mut buf_reader, &elem_block_header)?;
            let v8_elem = V8Elem::new().with_header(elem_block_data);

            let mut block_data = vec![];

            if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                let block_header_data = BlockHeader::from_raw_parts(&mut buf_reader)?;

                block_data = single::read_block_data(&mut buf_reader, &block_header_data)?;
            }

            if sender
                .send(RawData {
                    v8_elem,
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
        let elem_name = item.v8_elem.get_name()?;

        let mut file_elem_header = String::new();
        file_elem_header.push_str(&elem_name);
        file_elem_header.push_str(".header");

        fs::File::create(p_dir.join(&file_elem_header))?.write_all(&item.v8_elem.get_header())?;

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

    let (file_header, elems_addrs) = read_content(file_name)?;
    fs::File::create(p_dir.join("FileHeader"))?.write_all(&file_header.into_bytes()?)?;

    let (rawdata, h1) = start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);

    let result = start_file_write(rawdata, p_dir);

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
    let elems_addrs = single::read_elems_addrs(&mut buf_reader, &first_block_header)?;

    Ok((file_header, elems_addrs))
}

struct RawData {
    v8_elem: V8Elem,
    block_data: Vec<u8>,
}
