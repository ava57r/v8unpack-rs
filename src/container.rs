use std::fs;
use std::io::{BufReader, Cursor, Error as ioError, ErrorKind as ioErrorKind, Result, SeekFrom};
use std::io::prelude::*;
use std::path;

use byteorder::{LittleEndian, ReadBytesExt};
use std::convert::AsMut;

use std::u32;
use std::str;
use std::cmp;

use std::fmt;

use inflate;

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
            return Err(ioError::new(
                ioErrorKind::InvalidData,
                "Слишком мало байт прочитано",
            ));
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
            data_size_hex,
            page_size_hex,
            next_page_addr_hex
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
            return Err(ioError::new(
                ioErrorKind::InvalidData,
                "Слишком мало байт прочитано",
            ));
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

        if let Ok(res) = u32::from_str_radix(&s, 16) {
            return res;
        } else {
            return 0;
        }
    }
}

#[derive(Debug)]
pub struct ElemAddr {
    elem_header_addr: u32,
    elem_data_addr: u32,
    fffffff: u32, //всегда 0x7fffffff
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

    pub fn from_raw_parts(rdr: &mut Cursor<Vec<u8>>) -> Result<Self> {
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
    header: Vec<u8>,
}

impl V8Elem {
    pub fn get_name(&self) -> String {
        let (_, raw_name) = self.header.split_at(ElemHeaderBegin::SIZE as usize);
        let mut v_raw_name: Vec<u8> = vec![];

        for (i, ch) in raw_name.iter().enumerate() {
            if i % 2 == 0 {
                if *ch != b'\0' {
                    v_raw_name.push(*ch);
                }
            }
        }

        String::from_utf8(v_raw_name).unwrap()
    }
}

pub struct V8File;

impl V8File {
    pub fn unpack_to_directory_no_load(
        file_name: &str,
        dir_name: &str,
        bool_inflate: bool,
        _unpack_when_need: bool,
    ) -> Result<bool> {
        let file = fs::File::open(file_name)?;
        let mut buf_reader = BufReader::new(file);

        let _fh = FileHeader::from_raw_parts(&mut buf_reader)?;
        let bh = BlockHeader::from_raw_parts(&mut buf_reader)?;

        if !bh.is_correct() {
            return Ok(false);
        }

        let p_dir = path::Path::new(dir_name);
        if !p_dir.exists() {
            fs::create_dir(dir_name)?;
        }

        // Читаем элементы
        let block_data = V8File::read_block_data(&mut buf_reader, &bh)?;
        let data_size = block_data.len() as u64;
        let mut rdr = Cursor::new(block_data);

        let mut elems_addrs: Vec<ElemAddr> = vec![];

        while rdr.position() < data_size {
            elems_addrs.push(ElemAddr::from_raw_parts(&mut rdr)?);
        }

        for cur_elem in elems_addrs.iter() {
            if cur_elem.fffffff != V8_MAGIC_NUMBER {
                break;
            }

            buf_reader.seek(SeekFrom::Start(cur_elem.elem_header_addr as u64))?;

            let elem_block_header = BlockHeader::from_raw_parts(&mut buf_reader)?;

            if !elem_block_header.is_correct() {
                return Err(ioError::new(
                    ioErrorKind::Other,
                    "Прочитанный блок не является корректным v8_header",
                ));
            }

            let elem_block_data = V8File::read_block_data(&mut buf_reader, &elem_block_header)?;
            let elem = V8Elem {
                header: elem_block_data,
            };
            let elem_name = elem.get_name();

            let elem_path = p_dir.join(&elem_name);

            if cur_elem.elem_data_addr != V8_MAGIC_NUMBER {
                buf_reader.seek(SeekFrom::Start(cur_elem.elem_data_addr as u64))?;
                let _result = V8File::process_data(&mut buf_reader, bool_inflate, &elem_path)?;
            }
        }
        Ok(true)
    }

    pub fn read_block_data(
        src: &mut BufReader<fs::File>,
        block_header: &BlockHeader,
    ) -> Result<Vec<u8>> {
        let mut result: Vec<u8> = vec![];

        let data_size = block_header.get_data_size();
        let mut read_in_bytes = 0;

        let mut local_block_header = block_header.clone();
        while read_in_bytes < data_size {
            let page_size = local_block_header.get_page_size();
            let next_page_addr = local_block_header.get_next_page_addr();

            let bytes_to_read = cmp::min(page_size, data_size - read_in_bytes);
            let mut lbuf: Vec<u8> = vec![];
            let read_b = src.take(bytes_to_read as u64).read_to_end(&mut lbuf)?;

            read_in_bytes += bytes_to_read;
            if read_b < bytes_to_read as usize {
                return Err(ioError::new(
                    ioErrorKind::Other,
                    "Прочитано слишком мало байт",
                ));
            }

            result.extend(lbuf.iter());
            lbuf.clear();

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
            return Ok(false);
        }

        let block_data = V8File::read_block_data(src, &header)?;
        let out_data = match inflate::inflate_bytes(&block_data) {
            Ok(inf_bytes) => inf_bytes,
            Err(_) => block_data,
        };

        let is_v8file = {
            let mut rdr = Cursor::new(&out_data);
            V8File::is_v8file(&mut rdr)
        };

        if is_v8file {
            // TODO:
        } else {
            let mut elem_file = fs::File::create(elem_path.as_path())?;
            elem_file.write_all(&out_data)?;
        }

        Ok(true)
    }

    pub fn is_v8file<R>(reader: &mut R) -> bool
    where
        R: Read + Seek,
    {
        let _file_header = match FileHeader::from_raw_parts(reader) {
            Ok(header) => header,
            Err(_) => return false,
        };

        let block_header = match BlockHeader::from_raw_parts(reader) {
            Ok(header) => header,
            Err(_) => return false,
        };

        block_header.is_correct()
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
