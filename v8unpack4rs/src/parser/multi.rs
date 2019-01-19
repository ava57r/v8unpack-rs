use crate::container::*;
use crate::error;

use std::io::prelude::*;
use std::io::{BufReader, Cursor, SeekFrom};
use std::sync::mpsc::{sync_channel, Receiver};
use std::thread::{spawn, JoinHandle};
use std::{fs, path, str};

use super::single;
use inflate;
use log::*;

fn start_inflate_thread(
    v8_elems: Receiver<V8Elem>,
) -> (Receiver<V8Elem>, JoinHandle<Result<()>>) {
    let (sender, receiver) = sync_channel(128);

    let handle = spawn(move || {
        for v8_elem in v8_elems {
            let mut out_element = v8_elem;

            let mut inf_bytes_tuple = (vec![], false);

            if let Some(block_data) = out_element.get_data() {
                match inflate::inflate_bytes(&block_data) {
                    Ok(bytes) => {
                        inf_bytes_tuple.0 = bytes;
                        inf_bytes_tuple.1 = true;
                    }
                    Err(_) => {
                        inf_bytes_tuple.1 = false;
                    }
                }
            }

            if inf_bytes_tuple.1 {
                out_element = out_element.with_data(inf_bytes_tuple.0);
            }

            if sender.send(out_element).is_err() {
                break;
            }
        }
        Ok(())
    });

    (receiver, handle)
}

fn start_file_parse(
    v8_elems: Receiver<V8Elem>,
    p_dir: &path::Path,
    bool_inflate: bool,
) -> Result<bool> {
    for v8_elem in v8_elems {
        let name = v8_elem.get_name()?;
        info!("parse element {}", name);
        let elem_path = p_dir.join(name);

        if let Some(out_data) = v8_elem.get_data() {
            let mut rdr = Cursor::new(&out_data);
            if rdr.is_v8file() {
                single::load_file(&mut rdr, bool_inflate)?
                    .save_file_to_folder(&elem_path)?;
            } else {
                fs::File::create(elem_path.as_path())?.write_all(&out_data)?;
            }
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn parse_to_folder(
    file_name: &str,
    dir_name: &str,
    bool_inflate: bool,
) -> Result<bool> {
    let p_dir = path::Path::new(dir_name);
    if !p_dir.exists() {
        fs::create_dir(dir_name)?;
    };

    info!("the beginning of the file parsing {}", file_name);
    let (_, elems_addrs) = read_content(file_name)?;
    let (v8_elems, h1) =
        start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);
    let (inf_data, h2) = start_inflate_thread(v8_elems);

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
) -> (Receiver<V8Elem>, JoinHandle<Result<()>>) {
    let (sender, receiver) = sync_channel(128);

    let handle = spawn(move || {
        let file = fs::File::open(file_name)?;
        let mut buf_reader = BufReader::new(file);

        for cur_elem in elems_addrs.iter() {
            debug!("{:?}", cur_elem);
            if cur_elem.fffffff != V8_MAGIC_NUMBER {
                break;
            }

            let pos = buf_reader
                .seek(SeekFrom::Start(u64::from(cur_elem.elem_header_addr)))?;
            let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;
            if !elem_block_header.is_correct() {
                error!("the file is not in the correct format");
                return Err(error::V8Error::NotV8File { offset: pos });
            }

            let elem_block_data =
                single::read_block_data(&mut buf_reader, &elem_block_header)?;
            let mut v8_elem = V8Elem::new().with_header(elem_block_data);

            if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                buf_reader.seek(SeekFrom::Start(u64::from(cur_elem.elem_data_addr)))?;
                let block_header_data = BlockHeader::from_raw_parts(&mut buf_reader)?;

                v8_elem = v8_elem.with_data(single::read_block_data(
                    &mut buf_reader,
                    &block_header_data,
                )?);
            }

            if sender.send(v8_elem).is_err() {
                break;
            }
        }
        Ok(())
    });

    (receiver, handle)
}

fn start_file_write(v8_elems: Receiver<V8Elem>, p_dir: &path::Path) -> Result<bool> {
    for v8_elem in v8_elems {
        let elem_name = v8_elem.get_name()?;

        let file_elem_header = format!("{0}.{1}", elem_name, "header");
        info!("write to file {}", file_elem_header);
        fs::File::create(p_dir.join(&file_elem_header))?
            .write_all(&v8_elem.get_header())?;

        let file_elem_data = format!("{0}.{1}", elem_name, "data");
        info!("write to file {}", file_elem_header);
        if let Some(block_data) = v8_elem.get_data() {
            fs::File::create(p_dir.join(&file_elem_data))?.write_all(block_data)?;
        }
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

    let (v8_elems, h1) =
        start_file_reader_thread(path::PathBuf::from(file_name), elems_addrs);

    let result = start_file_write(v8_elems, p_dir);

    let r1 = h1.join().unwrap();

    r1?;

    result
}

fn read_content(file_name: &str) -> Result<(FileHeader, Vec<ElemAddr>)> {
    let file = fs::File::open(file_name)?;
    let mut buf_reader = BufReader::new(file);
    if !buf_reader.is_v8file() {
        error!("the file is not in the correct format");
        return Err(error::V8Error::NotV8File {
            offset: buf_reader.seek(SeekFrom::Current(0))?,
        });
    }

    let file_header = buf_reader.get_file_header()?;
    let first_block_header = buf_reader.get_first_block_header()?;
    let elems_addrs = single::read_elems_addrs(&mut buf_reader, &first_block_header)?;

    Ok((file_header, elems_addrs))
}
