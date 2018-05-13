use container::*;
use std::io::{Read, Write};
use std::{cmp, fs, path, ffi::OsStr, u32};

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
    ).expect("SaveFile. Error in creating file!");

    let mut file_out = fs::OpenOptions::new()
        .append(true)
        .open(filename_out)?;
    let pack_elements = prepare_pack_files(dirname)?;

    save_elem_addrs(&pack_elements, &mut file_out)?;
    save_data(pack_elements, &mut file_out)?;

    Ok(true)
}

fn save_elem_addrs(
    pack_elems: &Vec<PackElementEntry>,
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
        if pack_elem.header_size > u32::MAX as u64 {
            panic!("Invalid header length");
        }
        cur_elem_addr += BlockHeader::SIZE + pack_elem.header_size as u32;

        let elem_data_addr = cur_elem_addr;
        cur_elem_addr += BlockHeader::SIZE;
        if pack_elem.data_size > u32::MAX as u64 {
            panic!("Invalid data length");
        }
        cur_elem_addr += cmp::max(pack_elem.data_size as u32, V8_DEFAULT_PAGE_SIZE);

        elem_addrs_bytes
            .extend(ElemAddr::new(elem_data_addr, elem_header_addr).into_bytes()?);
    }

    save_block_data(file_out, elem_addrs_bytes, V8_DEFAULT_PAGE_SIZE)?;

    Ok(())
}

fn save_data(pack_elems: Vec<PackElementEntry>, file_out: &mut fs::File) -> Result<()> {
    for elem in pack_elems {
        {
            let mut header_file = fs::File::open(elem.header_file)?;
            let mut buf = vec![];
            header_file.read_to_end(&mut buf)?;
            save_block_data(file_out, buf, elem.header_size as u32)?;
        }
        {
            let mut data_file = fs::File::open(elem.data_file)?;
            let mut buf = vec![];
            data_file.read_to_end(&mut buf)?;
            save_block_data(file_out, buf, V8_DEFAULT_PAGE_SIZE)?;
        }
    }

    Ok(())
}

fn save_block_data(
    file_out: &mut fs::File,
    block_data: Vec<u8>,
    page_size: u32,
) -> Result<()> {
    if block_data.len() > u32::MAX as usize {
        panic!("Invalid data length");
    }

    let block_size = block_data.len() as u32;
    let page_size_actual = if page_size < block_size {
        block_size
    } else {
        page_size
    };

    let block_header = BlockHeader::new(block_size, page_size_actual, V8_MAGIC_NUMBER);

    file_out.write_all(&block_header.into_bytes()?)?;
    file_out.write_all(&block_data)?;
    let mut i = 0;
    while i < (page_size_actual - block_size) {
        file_out.write(b"\0")?;
        i += 1;
    }

    Ok(())
}
