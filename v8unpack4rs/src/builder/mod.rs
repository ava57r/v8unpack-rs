use crate::container::*;
use log::*;
use std::io::prelude::*;
use std::io::{Error as ioError, ErrorKind as ioErrorKind, Read, SeekFrom, Write};
use std::{cmp, ffi::OsStr, fs, path, u32};

#[derive(Debug)]
struct PackElementEntry {
    header_file: path::PathBuf,
    data_file: path::PathBuf,
    header_size: u64,
    data_size: u64,
}

fn prepare_pack_files(dirname: &str) -> Result<Vec<PackElementEntry>> {
    let files = fs::read_dir(dirname)?.filter(|p| {
        if let Ok(entry) = p {
            if let Some(ext) = entry.path().as_path().extension() {
                ext == OsStr::new("header")
            } else {
                false
            }
        } else {
            false
        }
    });

    let mut pack_elements = vec![];
    for file in files {
        if let Ok(entry) = file {
            let header_file = entry.path();
            let header_size = entry.metadata()?.len();

            let mut data_file = entry.path();
            data_file.set_extension(OsStr::new("data"));
            let data_size = fs::metadata(data_file.clone())?.len();

            pack_elements.push(PackElementEntry {
                header_file,
                data_file,
                header_size,
                data_size,
            });
        }
    }

    Ok(pack_elements)
}

/// assembling a container from a folder
pub fn pack_from_folder(dirname: &str, filename_out: &str) -> Result<bool> {
    fs::copy(
        path::Path::new(dirname).join("FileHeader"),
        path::Path::new(filename_out),
    )
    .expect("SaveFile. Error in creating file!");

    let mut file_out = fs::OpenOptions::new().append(true).open(filename_out)?;
    let pack_elements = prepare_pack_files(dirname)?;

    save_elem_addrs(&pack_elements, &mut file_out)?;
    save_data(pack_elements, &mut file_out)?;

    Ok(true)
}

fn save_elem_addrs(
    pack_elems: &[PackElementEntry],
    file_out: &mut fs::File,
) -> Result<()> {
    let mut elem_addrs_bytes: Vec<u8> =
        Vec::with_capacity(pack_elems.len() * ElemAddr::SIZE as usize);
    let mut cur_elem_addr = FileHeader::SIZE + BlockHeader::SIZE;

    cur_elem_addr += cmp::max(
        ElemAddr::SIZE * pack_elems.len() as u32,
        V8_DEFAULT_PAGE_SIZE,
    );

    for pack_elem in pack_elems {
        let elem_header_addr = cur_elem_addr;
        if pack_elem.header_size > u64::from(u32::MAX) {
            ioError::new(ioErrorKind::InvalidData, "Invalid header length");
        }
        cur_elem_addr += BlockHeader::SIZE + pack_elem.header_size as u32;

        let elem_data_addr = cur_elem_addr;
        cur_elem_addr += BlockHeader::SIZE;
        if pack_elem.data_size > u64::from(u32::MAX) {
            ioError::new(ioErrorKind::InvalidData, "Invalid data length");
        }
        cur_elem_addr += cmp::max(pack_elem.data_size as u32, V8_DEFAULT_PAGE_SIZE);

        elem_addrs_bytes
            .extend(ElemAddr::new(elem_data_addr, elem_header_addr).into_bytes()?);
    }

    save_block_data(file_out, &elem_addrs_bytes, V8_DEFAULT_PAGE_SIZE)?;

    Ok(())
}

fn save_data(pack_elems: Vec<PackElementEntry>, file_out: &mut fs::File) -> Result<()> {
    for elem in pack_elems {
        {
            let mut header_file = fs::File::open(elem.header_file)?;
            let mut buf = vec![];
            header_file.read_to_end(&mut buf)?;
            save_block_data(file_out, &buf, elem.header_size as u32)?;
        }
        {
            let mut data_file = fs::File::open(elem.data_file)?;
            let mut buf = vec![];
            data_file.read_to_end(&mut buf)?;
            save_block_data(file_out, &buf, V8_DEFAULT_PAGE_SIZE)?;
        }
    }

    Ok(())
}

fn save_block_data(
    file_out: &mut fs::File,
    block_data: &[u8],
    page_size: u32,
) -> Result<usize> {
    if block_data.len() > u32::MAX as usize {
        ioError::new(ioErrorKind::InvalidData, "Invalid data length");
    }

    let block_size = block_data.len() as u32;
    let page_size_actual = if page_size < block_size {
        block_size
    } else {
        page_size
    };

    let mut write_bytes: usize = 0;
    let block_header = BlockHeader::new(block_size, page_size_actual, V8_MAGIC_NUMBER);

    let bh_bytes = block_header.into_bytes()?;
    file_out.write_all(&bh_bytes)?;
    write_bytes += bh_bytes.len();
    file_out.write_all(&block_data)?;
    write_bytes += block_data.len();

    write_terminal_zeros(file_out, page_size_actual - block_size)?;
    write_bytes += (page_size_actual - block_size) as usize;

    Ok(write_bytes)
}

fn write_terminal_zeros(file_out: &mut fs::File, count: u32) -> Result<()> {
    let mut i = 0;
    while i < count {
        file_out.write_all(b"\0")?;
        i += 1;
    }

    Ok(())
}

pub fn build_cf_file(
    dirname: &str,
    filename_out: &str,
    no_deflate: bool,
) -> Result<bool> {
    let elems_num: u32 = fs::read_dir(dirname)?
        .filter(|p| p.is_ok())
        .fold(0, |sum, _| sum + 1);
    let mut toc: Vec<ElemAddr> = Vec::with_capacity(elems_num as usize);
    let mut cur_block_addr = FileHeader::SIZE + BlockHeader::SIZE;
    cur_block_addr += cmp::max(ElemAddr::SIZE * elems_num, V8_DEFAULT_PAGE_SIZE);

    let mut file_out = fs::File::create(filename_out)?;
    write_terminal_zeros(&mut file_out, cur_block_addr)?;
    toc.extend(process_files(
        dirname,
        &mut file_out,
        cur_block_addr,
        no_deflate,
    )?);

    let file_header = FileHeader::new(V8_MAGIC_NUMBER, V8_DEFAULT_PAGE_SIZE, 0);
    file_out.seek(SeekFrom::Start(0))?;
    file_out.write_all(&file_header.into_bytes()?)?;
    let mut toc_bytes = vec![];
    for toc_elm in toc.into_iter() {
        toc_bytes.extend(toc_elm.into_bytes()?);
    }
    save_block_data(&mut file_out, &toc_bytes, toc_bytes.len() as u32)?;

    Ok(true)
}

fn process_files(
    dirname: &str,
    file_out: &mut fs::File,
    cur_block_addr: u32,
    no_deflate: bool,
) -> Result<Vec<ElemAddr>> {
    let mut result = vec![];
    let mut cur_block_addr = cur_block_addr;
    for entry in fs::read_dir(dirname)? {
        let entry = entry?;
        if let Ok(name) = entry.file_name().into_string() {
            let header = vec![0; ElemHeaderBegin::SIZE as usize];
            let mut element = V8Elem::new().with_header(header);
            element.set_name(&name);

            let elem_header_addr = cur_block_addr;
            {
                let elem_header = element.get_header();
                cur_block_addr +=
                    save_block_data(file_out, elem_header, elem_header.len() as u32)?
                        as u32;
            }
            let elem_data_addr = cur_block_addr;

            let elem_addr = ElemAddr::new(elem_data_addr, elem_header_addr);
            result.push(elem_addr);

            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    process_directory(
                        file_out,
                        &mut element,
                        dirname,
                        &name,
                        no_deflate,
                        &mut cur_block_addr,
                    )?;
                } else {
                    process_v8file(
                        file_out,
                        &mut element,
                        dirname,
                        &name,
                        no_deflate,
                        &mut cur_block_addr,
                    )?;
                }
            } else {
                error!("Couldn't get file type for {:?}", entry.path());
            }
        } else {
            error!("Couldn't get file name for {:?}", entry.path());
        }
    }

    Ok(result)
}
fn process_directory(
    file_out: &mut fs::File,
    element: &mut V8Elem,
    dirname: &str,
    name: &str,
    no_deflate: bool,
    cur_elem_addr: &mut u32,
) -> Result<()> {
    let new_dir = path::Path::new(dirname).join(name);
    let mut v8 = V8File::new();
    v8.load_file_from_folder(new_dir)?;
    element.set_v8file(true);
    element.set_unpacked_data(Some(v8));
    element.pack(!no_deflate)?;

    if let Some(data) = element.get_data() {
        *cur_elem_addr += save_block_data(file_out, data, data.len() as u32)? as u32;
    }

    Ok(())
}

fn process_v8file(
    file_out: &mut fs::File,
    element: &mut V8Elem,
    dirname: &str,
    name: &str,
    no_deflate: bool,
    cur_block_addr: &mut u32,
) -> Result<()> {
    element.set_v8file(false);
    let mut data = vec![];
    let p_file = path::Path::new(dirname).join(name);
    let mut cur_file = fs::File::open(p_file)?;
    cur_file.read_to_end(&mut data)?;

    element.set_data(Some(data));
    element.pack(!no_deflate)?;

    if let Some(data) = element.get_data() {
        *cur_block_addr += save_block_data(file_out, data, data.len() as u32)? as u32;
    }

    Ok(())
}
