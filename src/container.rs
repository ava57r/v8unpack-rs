use std::{fmt, fs, path, result, str, u32};
use std::io::{Cursor, Error as ioError, ErrorKind as ioErrorKind, SeekFrom};
use std::io::prelude::*;

use byteorder::{LittleEndian, ReadBytesExt};
use std::convert::AsMut;

use error;

pub type Result<T> = result::Result<T, error::V8Error>;

pub const V8_MAGIC_NUMBER: u32 = 0x7fffffff;

#[derive(Debug, Default)]
#[repr(C)]
pub struct FileHeader {
    next_page_addr: u32,
    page_size: u32,
    storage_ver: u32,
    reserved: u32,
}

impl FileHeader {
    pub const SIZE: u32 = 4 + 4 + 4 + 4;

    pub fn new(next_page_addr: u32, page_size: u32, storage_ver: u32) -> FileHeader {
        FileHeader {
            next_page_addr: next_page_addr,
            page_size: page_size,
            storage_ver: storage_ver,
            reserved: 0,
        }
    }

    pub fn from_raw_parts<R>(src: &mut R) -> Result<FileHeader>
    where
        R: Read + Seek,
    {
        let mut buf = vec![];
        let read_bytes = src.take(Self::SIZE as u64).read_to_end(&mut buf)?;
        if read_bytes < Self::SIZE as usize {
            return Err(error::V8Error::IoError(ioError::new(
                ioErrorKind::InvalidData,
                "Слишком мало байт прочитано",
            )));
        }

        let mut rdr = Cursor::new(buf);
        let _next_page_addr = rdr.read_u32::<LittleEndian>()?;
        let _page_size = rdr.read_u32::<LittleEndian>()?;
        let _storage_ver = rdr.read_u32::<LittleEndian>()?;
        let _reserved = rdr.read_u32::<LittleEndian>()?;

        Ok(FileHeader {
            next_page_addr: _next_page_addr,
            page_size: _page_size,
            storage_ver: _storage_ver,
            reserved: _reserved,
        })
    }
}

#[derive(Debug, Copy)]
pub struct BlockHeader {
    eol_0d: u8,
    eol_0a: u8,
    data_size_hex: [u8; 8],
    space1: u8,
    page_size_hex: [u8; 8],
    space2: u8,
    next_page_addr_hex: [u8; 8],
    space3: u8,
    eol2_0d: u8,
    eol2_0a: u8,
}

impl Default for BlockHeader {
    fn default() -> BlockHeader {
        BlockHeader {
            eol_0d: b'\r',
            eol_0a: b'\n',
            data_size_hex: [0; 8],
            space1: b'\x20',
            page_size_hex: [0; 8],
            space2: b'\x20',
            next_page_addr_hex: [0; 8],
            space3: b'\x20',
            eol2_0d: b'\r',
            eol2_0a: b'\n',
        }
    }
}

impl Clone for BlockHeader {
    fn clone(&self) -> BlockHeader {
        *self
    }
}

impl fmt::Display for BlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let data_size_hex = str::from_utf8(&self.data_size_hex).unwrap();
        let page_size_hex = str::from_utf8(&self.page_size_hex).unwrap();
        let next_page_addr_hex = str::from_utf8(&self.next_page_addr_hex).unwrap();

        write!(
            f,
            "data_size_hex: {}\npage_size_hex: {}\nnext_page_addr_hex: {}",
            data_size_hex, page_size_hex, next_page_addr_hex
        )
    }
}

impl BlockHeader {
    pub const SIZE: u32 = 1 + 1 + 8 + 1 + 8 + 1 + 8 + 1 + 1 + 1;

    pub fn from_raw_parts<R>(src: &mut R) -> Result<BlockHeader>
    where
        R: Read + Seek,
    {
        let mut buf = vec![];
        let read_bytes = src.take(Self::SIZE as u64).read_to_end(&mut buf)?;
        if read_bytes < Self::SIZE as usize {
            return Err(error::V8Error::IoError(ioError::new(
                ioErrorKind::InvalidData,
                "Слишком мало байт прочитано",
            )));
        }

        let mut rdr = Cursor::new(buf);
        let _eol_0d = rdr.read_u8()?;
        let _eol_oa = rdr.read_u8()?;

        let _data_size_hex = clone_into_array(&rdr.get_ref()[2..10]);

        rdr.set_position(10);
        let _space1 = rdr.read_u8()?;

        let _page_size_hex = clone_into_array(&rdr.get_ref()[11..19]);

        rdr.set_position(19);
        let _space2 = rdr.read_u8()?;

        let mut _next_page_addr_hex = clone_into_array(&rdr.get_ref()[20..28]);

        rdr.set_position(28);
        let _space3 = rdr.read_u8()?;
        let _eol2_0d = rdr.read_u8()?;
        let _eol2_oa = rdr.read_u8()?;

        Ok(BlockHeader {
            eol_0d: _eol_0d,
            eol_0a: _eol_oa,
            data_size_hex: _data_size_hex,
            space1: _space1,
            page_size_hex: _page_size_hex,
            space2: _space2,
            next_page_addr_hex: _next_page_addr_hex,
            space3: _space3,
            eol2_0d: _eol2_0d,
            eol2_0a: _eol2_oa,
        })
    }

    pub fn is_correct(&self) -> bool {
        self.eol_0d == b'\r' && self.eol_0a == b'\n' && self.space1 == b'\x20'
            && self.space2 == b'\x20' && self.space3 == b'\x20' && self.eol2_0d == b'\r'
            && self.eol2_0a == b'\n'
    }

    pub fn get_data_size(&self) -> u32 {
        Self::get_u32(&self.data_size_hex)
    }

    pub fn get_page_size(&self) -> u32 {
        Self::get_u32(&self.page_size_hex)
    }

    pub fn get_next_page_addr(&self) -> u32 {
        Self::get_u32(&self.next_page_addr_hex)
    }

    fn get_u32(value: &[u8]) -> u32 {
        let s = match str::from_utf8(value) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };

        u32::from_str_radix(s, 16).unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct ElemAddr {
    pub elem_header_addr: u32,
    pub elem_data_addr: u32,
    pub fffffff: u32, //всегда 0x7fffffff ?
}

impl ElemAddr {
    pub const SIZE: u32 = 4 + 4 + 4;

    pub fn new(elem_data_addr: u32, elem_header_addr: u32) -> Self {
        ElemAddr {
            elem_header_addr: elem_header_addr,
            elem_data_addr: elem_data_addr,
            fffffff: V8_MAGIC_NUMBER,
        }
    }

    pub fn from_raw_parts<R>(rdr: &mut R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let _elem_header_addr = rdr.read_u32::<LittleEndian>()?;
        let _elem_data_addr = rdr.read_u32::<LittleEndian>()?;
        let _fffffff = rdr.read_u32::<LittleEndian>()?;

        Ok(ElemAddr {
            elem_header_addr: _elem_header_addr,
            elem_data_addr: _elem_data_addr,
            fffffff: _fffffff,
        })
    }
}

impl Default for ElemAddr {
    fn default() -> ElemAddr {
        ElemAddr {
            elem_header_addr: 0,
            elem_data_addr: 0,
            fffffff: 0,
        }
    }
}

#[allow(dead_code)]
struct ElemHeaderBegin {
    date_creation: u64,
    date_modification: u64,
    res: u32,
}

impl ElemHeaderBegin {
    pub const SIZE: u32 = 8 + 8 + 4;
}

pub struct V8Elem {
    pub header: Vec<u8>,
    pub data: Option<Vec<u8>>,
    pub unpacked_data: Option<V8File>,
    pub is_v8file: bool,
}

impl V8Elem {
    pub fn get_name(&self) -> Result<String> {
        let (_, raw_name) = self.header.split_at(ElemHeaderBegin::SIZE as usize);
        let mut v_raw_name: Vec<u8> = vec![];

        for (i, ch) in raw_name.iter().enumerate() {
            if i % 2 == 0 {
                if *ch != b'\0' {
                    v_raw_name.push(*ch);
                }
            }
        }

        Ok(String::from_utf8(v_raw_name)?)
    }
}

pub struct V8File {
    file_header: FileHeader,
    elems_addrs: Vec<ElemAddr>,
    elems: Vec<V8Elem>,
}

impl V8File {
    pub fn new() -> V8File {
        V8File {
            file_header: FileHeader::default(),
            elems_addrs: vec![],
            elems: vec![],
        }
    }

    pub fn with_header(mut self, header: FileHeader) -> V8File {
        self.file_header = header;

        self
    }

    pub fn with_elems_addrs(mut self, elems: Vec<ElemAddr>) -> V8File {
        self.elems_addrs = elems;

        self
    }

    pub fn with_elems(mut self, elems: Vec<V8Elem>) -> V8File {
        self.elems = elems;

        self
    }

    pub fn save_file_to_folder(&self, elem_path: &path::PathBuf) -> Result<bool> {
        if !elem_path.exists() {
            fs::create_dir(elem_path.as_path())?;
        }

        for elem in self.elems.iter() {
            let name_elem = elem.get_name()?;

            let out_path = elem_path.join(name_elem);

            if !elem.is_v8file {
                if let Some(out_data) = elem.data.as_ref() {
                    let mut filename_out = fs::File::create(out_path.as_path())?;
                    filename_out.write_all(out_data)?;
                }
            } else {
                if let Some(out_file) = elem.unpacked_data.as_ref() {
                    out_file.save_file_to_folder(&out_path)?;
                }
            }
        }

        Ok(true)
    }
}

// (c) https://stackoverflow.com/questions/25428920/how-to-get-a-slice-as-an-array-in-rust
fn clone_into_array<A, T>(slice: &[T]) -> A
where
    A: Sized + Default + AsMut<[T]>,
    T: Clone,
{
    let mut a = Default::default();
    <A as AsMut<[T]>>::as_mut(&mut a).clone_from_slice(slice);
    a
}
