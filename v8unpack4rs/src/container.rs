use std::io::prelude::*;
use std::io::{BufReader, Cursor, Error as ioError, ErrorKind as ioErrorKind, SeekFrom};
use std::{cmp, fmt, fs, path, result, str, u32};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::convert::AsMut;

use encoding::all::UTF_16LE;
use encoding::{EncoderTrap, Encoding};

use deflate;
use log::*;

use crate::error;

pub type Result<T> = result::Result<T, error::V8Error>;

pub const V8_DEFAULT_PAGE_SIZE: u32 = 512;

/// Indicates that no further data.
pub const V8_MAGIC_NUMBER: u32 = 0x7fff_ffff;

/// Trait for to get basic information about the container.
pub trait V8Container {
    /// This method checks that the container is actually the correct file of
    /// 1C: Enterprise.
    fn is_v8file(&mut self) -> bool;

    /// Returns root file header from container.
    fn get_file_header(&mut self) -> Result<FileHeader>;

    /// Returns first block from container.
    fn get_first_block_header(&mut self) -> Result<BlockHeader>;
}

impl<T> V8Container for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn is_v8file(&mut self) -> bool {
        self.set_position(0);

        let _file_header = match FileHeader::from_raw_parts(self) {
            Ok(header) => header,
            Err(_) => return false,
        };

        let block_header = match BlockHeader::from_raw_parts(self) {
            Ok(header) => header,
            Err(_) => return false,
        };

        block_header.is_correct()
    }

    fn get_file_header(&mut self) -> Result<FileHeader> {
        self.set_position(0);

        FileHeader::from_raw_parts(self)
    }

    fn get_first_block_header(&mut self) -> Result<BlockHeader> {
        self.set_position(u64::from(FileHeader::SIZE));

        BlockHeader::from_raw_parts(self)
    }
}

impl V8Container for BufReader<fs::File> {
    fn is_v8file(&mut self) -> bool {
        if self.seek(SeekFrom::Start(0)).is_err() {
            return false;
        }

        let _file_header = match FileHeader::from_raw_parts(self) {
            Ok(header) => header,
            Err(_) => return false,
        };

        let block_header = match BlockHeader::from_raw_parts(self) {
            Ok(header) => header,
            Err(_) => return false,
        };

        block_header.is_correct()
    }

    fn get_file_header(&mut self) -> Result<FileHeader> {
        self.seek(SeekFrom::Start(0))?;

        FileHeader::from_raw_parts(self)
    }

    fn get_first_block_header(&mut self) -> Result<BlockHeader> {
        self.seek(SeekFrom::Start(u64::from(FileHeader::SIZE)))?;

        BlockHeader::from_raw_parts(self)
    }
}

/// Describes the structure of the header of the container file.
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct FileHeader {
    next_page_addr: u32,
    page_size: u32,
    storage_ver: u32,
    reserved: u32,
}

impl FileHeader {
    /// The size of the data in the file, represented as C structures
    pub const SIZE: u32 = 4 + 4 + 4 + 4;

    pub fn new(next_page_addr: u32, page_size: u32, storage_ver: u32) -> FileHeader {
        FileHeader {
            next_page_addr,
            page_size,
            storage_ver,
            reserved: 0,
        }
    }

    pub fn from_raw_parts<R>(src: &mut R) -> Result<FileHeader>
    where
        R: Read + Seek,
    {
        let mut buf = vec![];
        let read_bytes = src.take(u64::from(Self::SIZE)).read_to_end(&mut buf)?;
        if read_bytes < Self::SIZE as usize {
            return Err(error::V8Error::IoError(ioError::new(
                ioErrorKind::InvalidData,
                "Readied too few bytes",
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

    pub fn into_bytes(self) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        result.write_u32::<LittleEndian>(self.next_page_addr)?;
        result.write_u32::<LittleEndian>(self.page_size)?;
        result.write_u32::<LittleEndian>(self.storage_ver)?;
        result.write_u32::<LittleEndian>(self.reserved)?;

        Ok(result)
    }
}

/// Describes the structure of header data block.
/// Example empty block header `\r\n00000000 00000000 00000000 \r\n`.
#[derive(Debug, Clone)]
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

fn convert(value: u32) -> [u8; 8] {
    let hex = format!("{:08x}", value);
    let bytes = hex.into_bytes();
    let arr: [u8; 8] = clone_into_array(&bytes[0..8]);

    arr
}

impl BlockHeader {
    /// The size of the data in the file, represented as C structures.
    pub const SIZE: u32 = 1 + 1 + 8 + 1 + 8 + 1 + 8 + 1 + 1 + 1;

    pub fn new(data_size: u32, page_size: u32, next_page_addr: u32) -> BlockHeader {
        let mut default = BlockHeader::default();

        default.data_size_hex = convert(data_size);
        default.page_size_hex = convert(page_size);
        default.next_page_addr_hex = convert(next_page_addr);

        default
    }
    /// Creates an instance of `BlockHeader` from a stream of bytes.
    pub fn from_raw_parts<R>(src: &mut R) -> Result<BlockHeader>
    where
        R: Read + Seek,
    {
        let mut buf = vec![];
        let read_bytes = src.take(u64::from(Self::SIZE)).read_to_end(&mut buf)?;
        if read_bytes < Self::SIZE as usize {
            return Err(error::V8Error::IoError(ioError::new(
                ioErrorKind::InvalidData,
                "Readied too few bytes",
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

    /// Checks that the block header for correctness.
    pub fn is_correct(&self) -> bool {
        self.eol_0d == b'\r'
            && self.eol_0a == b'\n'
            && self.space1 == b'\x20'
            && self.space2 == b'\x20'
            && self.space3 == b'\x20'
            && self.eol2_0d == b'\r'
            && self.eol2_0a == b'\n'
    }

    /// Gets the value of the size of the data section from hexadecimal
    /// representation.
    pub fn get_data_size(&self) -> Result<u32> {
        Self::get_u32(&self.data_size_hex)
    }

    /// Gets the value of the page size data from hexadecimal representation.
    pub fn get_page_size(&self) -> Result<u32> {
        Self::get_u32(&self.page_size_hex)
    }

    /// Gets the offset of the next page of data from hexadecimal
    /// representation.
    pub fn get_next_page_addr(&self) -> Result<u32> {
        Self::get_u32(&self.next_page_addr_hex)
    }

    fn get_u32(value: &[u8]) -> Result<u32> {
        let s = str::from_utf8(&value)?;

        Ok(u32::from_str_radix(s, 16)?)
    }

    /// Converts `BlockHeader` an array of bytes
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        result.push(self.eol_0d);
        result.push(self.eol_0a);
        result.extend(self.data_size_hex.iter());
        result.push(self.space1);
        result.extend(self.page_size_hex.iter());
        result.push(self.space2);
        result.extend(self.next_page_addr_hex.iter());
        result.push(self.space3);
        result.push(self.eol2_0d);
        result.push(self.eol2_0a);

        Ok(result)
    }
}

/// Is the structure and arrangement of data partitions in the container.
#[derive(Debug, Default)]
pub struct ElemAddr {
    /// The offset into the file where is the header block.
    pub elem_header_addr: u32,
    /// The offset into the file where located data block.
    pub elem_data_addr: u32,
    /// Always equal `V8_MAGIC_NUMBER`.
    pub fffffff: u32, //always == 0x7fffffff ?
}

impl ElemAddr {
    /// The size of the data in the file, represented as C structures.
    pub const SIZE: u32 = 4 + 4 + 4;

    /// Creates a new instance of `ElemAddr`.
    pub fn new(elem_data_addr: u32, elem_header_addr: u32) -> Self {
        ElemAddr {
            elem_header_addr,
            elem_data_addr,
            fffffff: V8_MAGIC_NUMBER,
        }
    }

    /// Creates an instance of `ElemAddr` from a stream of bytes.
    pub fn from_raw_parts<R>(rdr: &mut R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let elem_header_addr = rdr.read_u32::<LittleEndian>()?;
        let elem_data_addr = rdr.read_u32::<LittleEndian>()?;
        let fffffff = rdr.read_u32::<LittleEndian>()?;

        Ok(ElemAddr {
            elem_header_addr,
            elem_data_addr,
            fffffff,
        })
    }

    /// Converts `ElemAddr` an array of bytes
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        result.write_u32::<LittleEndian>(self.elem_header_addr)?;
        result.write_u32::<LittleEndian>(self.elem_data_addr)?;
        result.write_u32::<LittleEndian>(self.fffffff)?;

        Ok(result)
    }
}

#[allow(dead_code)]
pub struct ElemHeaderBegin {
    date_creation: u64,
    date_modification: u64,
    res: u32,
}

impl ElemHeaderBegin {
    /// The size of the data in the file, represented as C structures.
    pub const SIZE: u32 = 8 + 8 + 4;
}

/// Describes the structure of the data item container.
#[derive(Debug, Default)]
pub struct V8Elem {
    header: Vec<u8>,
    data: Option<Vec<u8>>,
    unpacked_data: Option<V8File>,
    is_v8file: bool,
}

impl V8Elem {
    /// Creates a new instance of `V8Elem`.
    pub fn new() -> V8Elem {
        V8Elem::default()
    }

    pub fn with_header(mut self, value: Vec<u8>) -> Self {
        self.header = value;

        self
    }

    pub fn get_header(&self) -> &Vec<u8> {
        &self.header
    }

    pub fn with_data(mut self, value: Vec<u8>) -> Self {
        self.data = Some(value);

        self
    }

    pub fn get_data(&self) -> Option<&Vec<u8>> {
        self.data.as_ref()
    }

    pub fn set_data(&mut self, value: Option<Vec<u8>>) {
        self.data = value;
    }

    pub fn with_unpacked_data(mut self, value: V8File) -> Self {
        self.unpacked_data = Some(value);

        self
    }

    pub fn set_unpacked_data(&mut self, value: Option<V8File>) {
        self.unpacked_data = value;
    }

    ///
    pub fn this_v8file(mut self, value: bool) -> Self {
        self.is_v8file = value;

        self
    }

    pub fn get_v8file(&self) -> bool {
        self.is_v8file
    }

    pub fn set_v8file(&mut self, value: bool) {
        self.is_v8file = value;
    }

    /// Gets the name of the file in the container.
    pub fn get_name(&self) -> Result<String> {
        let (_, raw_name) = self.header.split_at(ElemHeaderBegin::SIZE as usize);
        let mut v_raw_name: Vec<u8> = vec![];

        for (i, ch) in raw_name.iter().enumerate() {
            if i % 2 == 0 && *ch != b'\0' {
                v_raw_name.push(*ch);
            }
        }

        Ok(String::from_utf8(v_raw_name)?)
    }

    pub fn set_name(&mut self, value: &str) {
        if let Ok(utf_16) = UTF_16LE.encode(value, EncoderTrap::Strict) {
            self.header.extend(utf_16.iter());
            self.header.push(b'\0');
            self.header.extend(&[0, 0, 0, 0]);
        }
    }

    pub fn pack(&mut self, deflate_: bool) -> Result<()> {
        if !self.is_v8file {
            if deflate_ {
                let result = match self.data {
                    Some(ref data) => deflate::deflate_bytes(data),
                    None => {
                        error!("Couldn't get data from V8Elem");

                        vec![]
                    }
                };

                self.set_data(Some(result));
            }
        } else {
            let data_buffer = match self.unpacked_data {
                Some(ref unpacked_data) => unpacked_data.get_data()?,
                None => {
                    error!("Couldn't get data from V8File");

                    vec![]
                }
            };
            self.set_unpacked_data(None);

            if deflate_ {
                let result = deflate::deflate_bytes(&data_buffer);
                self.set_data(Some(result));
            } else {
                self.set_data(Some(data_buffer));
            }
            self.is_v8file = false;
        }

        Ok(())
    }
}

/// Describes the structure of the file `1cd`.
#[derive(Debug, Default)]
pub struct V8File {
    /// The file header `1cd`.
    file_header: FileHeader,
    ///a collection of elements that describe offsets of the header and data
    /// sections.
    elems_addrs: Vec<ElemAddr>,
    ///
    elems: Vec<V8Elem>,
}

impl V8File {
    /// Creates a new instance of `V8File`.
    pub fn new() -> V8File {
        V8File::default()
    }

    pub fn with_header(mut self, value: FileHeader) -> Self {
        self.file_header = value;

        self
    }

    pub fn with_elems_addrs(mut self, value: Vec<ElemAddr>) -> Self {
        self.elems_addrs = value;

        self
    }

    pub fn with_elems(mut self, value: Vec<V8Elem>) -> Self {
        self.elems = value;

        self
    }

    /// Stores data in files on disk.
    pub fn save_file_to_folder(&self, elem_path: &path::PathBuf) -> Result<bool> {
        if !elem_path.exists() {
            fs::create_dir(elem_path.as_path())?;
        }

        for elem in self.elems.iter() {
            let name_elem = elem.get_name()?;
            info!("parse element {}", name_elem);
            let out_path = elem_path.join(name_elem);

            if !elem.is_v8file {
                if let Some(out_data) = elem.data.as_ref() {
                    let mut filename_out = fs::File::create(out_path.as_path())?;
                    filename_out.write_all(out_data)?;
                }
            } else if let Some(out_file) = elem.unpacked_data.as_ref() {
                out_file.save_file_to_folder(&out_path)?;
            }
        }

        Ok(true)
    }

    pub fn load_file_from_folder(&mut self, dirname: path::PathBuf) -> Result<()> {
        self.file_header = FileHeader::new(V8_MAGIC_NUMBER, V8_DEFAULT_PAGE_SIZE, 0);
        self.elems.clear();

        for entry in fs::read_dir(dirname.as_path())? {
            let entry = entry?;
            if let Ok(name) = entry.file_name().into_string() {
                let header = vec![0; ElemHeaderBegin::SIZE as usize];
                let mut element = V8Elem::new().with_header(header);
                element.set_name(&name);

                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let new_dir = dirname.join(name);
                        let mut v8 = V8File::new();
                        v8.load_file_from_folder(new_dir)?;
                        element.set_v8file(true);
                        element.set_unpacked_data(Some(v8));
                        element.pack(false)?;
                    } else {
                        element.set_v8file(false);
                        let mut file = fs::File::open(entry.path())?;
                        let mut buf = vec![];
                        file.read_to_end(&mut buf)?;
                        element.set_data(Some(buf));
                    }
                } else {
                    error!("Couldn't get file type for {:?}", entry.path());
                }
                self.elems.push(element);
            } else {
                error!("Couldn't get file name for {:?}", entry.path());
            }
        }

        Ok(())
    }

    pub fn get_data(&self) -> Result<Vec<u8>> {
        let mut result = vec![];
        let fh = self.file_header.clone();
        result.extend(fh.into_bytes()?);

        let mut elem_addrs_bytes: Vec<u8> =
            Vec::with_capacity(self.elems.len() * ElemAddr::SIZE as usize);

        let mut cur_elem_addr = FileHeader::SIZE + BlockHeader::SIZE;
        cur_elem_addr += cmp::max(
            ElemAddr::SIZE * self.elems.len() as u32,
            V8_DEFAULT_PAGE_SIZE,
        );

        let mut new_elems: Vec<V8Elem> = vec![];

        for elem in self.elems.iter() {
            if elem.get_v8file() {
                let data_buffer = match elem.unpacked_data {
                    Some(ref unpacked_data) => unpacked_data.get_data()?,
                    None => {
                        error!("Couldn't get data from V8File");

                        vec![]
                    }
                };

                new_elems.push(V8Elem {
                    header: elem.header.clone(),
                    data: Some(data_buffer),
                    unpacked_data: None,
                    is_v8file: false,
                });
            }
            let data_new = match elem.data {
                Some(ref data) => data.clone(),
                None => vec![],
            };

            new_elems.push(
                V8Elem::new()
                    .with_header(elem.header.clone())
                    .with_data(data_new),
            );
        }

        for elem in new_elems.iter() {
            let elem_header_addr = cur_elem_addr;
            cur_elem_addr += BlockHeader::SIZE + elem.header.len() as u32;

            let elem_data_addr = cur_elem_addr;
            cur_elem_addr += BlockHeader::SIZE;
            if let Some(ref data) = elem.data {
                cur_elem_addr += cmp::max(data.len() as u32, V8_DEFAULT_PAGE_SIZE);
            } else {
                error!("Empty!");
            }

            elem_addrs_bytes
                .extend(ElemAddr::new(elem_data_addr, elem_header_addr).into_bytes()?);
        }

        V8File::save_block_data_to_buffer(
            &mut result,
            &elem_addrs_bytes,
            V8_DEFAULT_PAGE_SIZE,
        )?;

        for elem in new_elems.iter() {
            V8File::save_block_data_to_buffer(
                &mut result,
                &elem.header,
                elem.header.len() as u32,
            )?;

            if let Some(ref data) = elem.data {
                V8File::save_block_data_to_buffer(
                    &mut result,
                    data,
                    cmp::max(data.len() as u32, V8_DEFAULT_PAGE_SIZE),
                )?;
            } else {
                error!("Empty!");
            }
        }

        Ok(result)
    }

    fn save_block_data_to_buffer(
        buffer: &mut Vec<u8>,
        block_data: &[u8],
        page_size: u32,
    ) -> Result<()> {
        if block_data.len() > u32::MAX as usize {
            ioError::new(ioErrorKind::InvalidData, "Invalid data length");
        }

        let block_size = block_data.len() as u32;
        let page_size_actual = if page_size < block_size {
            block_size
        } else {
            page_size
        };

        let block_header =
            BlockHeader::new(block_size, page_size_actual, V8_MAGIC_NUMBER);

        buffer.extend(&block_header.into_bytes()?);
        buffer.extend(block_data.iter());

        let mut i = 0;
        while i < (page_size_actual - block_size) {
            buffer.push(0);
            i += 1;
        }

        Ok(())
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
