use container::*;
use error;

use std::{cmp, fs, path, str};
use std::io::{BufReader, Cursor, Error as ioError, ErrorKind as ioErrorKind, SeekFrom};
use std::io::prelude::*;

use inflate;

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

    let elems_addrs = read_elems_addrs(&mut buf_reader, &first_block_header)?;

    for cur_elem in elems_addrs.iter() {
        if cur_elem.fffffff != V8_MAGIC_NUMBER {
            break;
        }

        buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

        let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;

        if !elem_block_header.is_correct() {
            return Err(error::V8Error::NotV8File);
        }

        let elem_block_data = read_block_data(&mut buf_reader, &elem_block_header)?;
        let elem_name = V8Elem::new().with_header(elem_block_data).get_name()?;

        let elem_path = p_dir.join(&elem_name);

        if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
            buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
            let _result = process_data(&mut buf_reader, bool_inflate, &elem_path)?;
        }
    }
    Ok(true)
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

    let elems_addrs = read_elems_addrs(&mut buf_reader, &first_block_header)?;

    for cur_elem in elems_addrs.iter() {
        if cur_elem.fffffff != V8_MAGIC_NUMBER {
            break;
        }

        buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

        let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;

        if !elem_block_header.is_correct() {
            return Err(error::V8Error::NotV8File);
        }

        let elem_block_data = read_block_data(&mut buf_reader, &elem_block_header)?;
        let v8_elem = V8Elem::new().with_header(elem_block_data);
        let elem_name = v8_elem.get_name()?;

        let mut file_elem_header = String::new();
        file_elem_header.push_str(&elem_name);
        file_elem_header.push_str(".header");

        fs::File::create(p_dir.join(&file_elem_header))?.write_all(&v8_elem.get_header())?;

        if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
            buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
            let block_header_data = BlockHeader::from_raw_parts(&mut buf_reader)?;

            let block_data = read_block_data(&mut buf_reader, &block_header_data)?;
            let mut file_elem_data = String::new();
            file_elem_data.push_str(&elem_name);
            file_elem_data.push_str(".data");
            fs::File::create(p_dir.join(&file_elem_data))?.write_all(&block_data)?;
        }
    }
    Ok(true)
}

pub fn read_elems_addrs<R>(src: &mut R, block_header: &BlockHeader) -> Result<Vec<ElemAddr>>
where
    R: Read + Seek,
{
    let block_data = read_block_data(src, block_header)?;
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

    let block_data = read_block_data(src, &header)?;
    let out_data = match inflate::inflate_bytes(&block_data) {
        Ok(inf_bytes) => inf_bytes,
        Err(_) => block_data,
    };

    let mut rdr = Cursor::new(&out_data);

    if rdr.is_v8file() {
        load_file(&mut rdr, _need_unpack)?.save_file_to_folder(elem_path)?;
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

    let elems_addrs = read_elems_addrs(src, &first_block_header)?;
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

        let elem_block_header_data = read_block_data(src, &elem_block_header)?;

        let elem_block_data: Vec<u8> = if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
            src.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
            let block_header_data = BlockHeader::from_raw_parts(src)?;

            read_block_data(src, &block_header_data)?
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
            load_file(&mut rdr, bool_inflate)?
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
