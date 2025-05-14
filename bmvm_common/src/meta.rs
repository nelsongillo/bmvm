use crate::hash::Djb2;
use anyhow::anyhow;
use std::ffi::CString;
use std::ops::Not;
use syn::{Type, TypePath, TypeReference, TypeSlice};

pub const STATIC_META_NAME_PREFIX: &str = "BMVM_CALL_HOST_META_";
pub const LINK_SECTION_META_NAME: &str = ".bmvm.call.host";

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Hash, Debug)]
pub enum DataType {
    UInt8 = 0,
    UInt16 = 1,
    UInt32 = 2,
    UInt64 = 3,
    Int8 = 4,
    Int16 = 5,
    Int32 = 6,
    Int64 = 7,
    Float32 = 8,
    Float64 = 9,
    Bytes = 10,
}

impl TryFrom<u8> for DataType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DataType::UInt8),
            1 => Ok(DataType::UInt16),
            2 => Ok(DataType::UInt32),
            3 => Ok(DataType::UInt64),
            4 => Ok(DataType::Int8),
            5 => Ok(DataType::Int16),
            6 => Ok(DataType::Int32),
            7 => Ok(DataType::Int64),
            8 => Ok(DataType::Float32),
            9 => Ok(DataType::Float64),
            10 => Ok(DataType::Bytes),
            _ => Err("Values greater than 10 can not be mapped!"),
        }
    }
}

impl TryFrom<Type> for DataType {
    type Error = &'static str;

    fn try_from(ty: Type) -> Result<Self, Self::Error> {
        match ty {
            // Match simple types like u32, i8, etc.
            Type::Path(TypePath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    match ident.to_string().as_str() {
                        "u8" => Ok(DataType::UInt8),
                        "u16" => Ok(DataType::UInt16),
                        "u32" => Ok(DataType::UInt32),
                        "u64" => Ok(DataType::UInt64),
                        "i8" => Ok(DataType::Int8),
                        "i16" => Ok(DataType::Int16),
                        "i32" => Ok(DataType::Int32),
                        "i64" => Ok(DataType::Int64),
                        "f32" => Ok(DataType::Float32),
                        "f64" => Ok(DataType::Float64),
                        _ => Err("Unsupported type"),
                    }
                } else {
                    Err("Unsupported type")
                }
            }

            // Match references: &T or &mut T
            Type::Reference(TypeReference { elem, .. }) => match *elem {
                // Match &[u8]
                Type::Slice(TypeSlice { elem, .. }) => {
                    if let Type::Path(TypePath { path, .. }) = *elem {
                        if let Some(ident) = path.get_ident() {
                            if ident == "u8" {
                                return Ok(DataType::Bytes);
                            }
                        }
                    }
                    Err("Unsupported type")
                }
                _ => Err("Unsupported type")
            },
            _ => Err("Unsupported type")
        }
    }
}

pub struct CallMeta {
    id: u32,
    argv: Vec<DataType>,
    fn_name: CString,
}

impl CallMeta {
    const MIN_SIZE: usize = {
        // u32 ID
        size_of::<u32>()
            // argc (here 0 -> no argv)
            + size_of::<u8>()
            // Fn Name: min len 1 + null terminator
            + size_of::<u8>() + size_of::<u8>()
    };

    const ARGV_BEGIN: usize = {
        // u32 ID
        size_of::<u32>()
            // argc (here 0 -> no argv)
            + size_of::<u8>()
    };

    pub fn new(argv: Vec<DataType>, fn_name: &str) -> anyhow::Result<Self> {
        if argv.len() > u8::MAX as usize {
            Err(anyhow!("argv must not be longer than u8::MAX"))?
        }

        let mut hasher = Djb2::new();
        hasher.write(fn_name.as_bytes());
        hasher.write(
            argv.iter()
                .map(|arg| arg.clone() as u8)
                .collect::<Vec<u8>>()
                .as_slice(),
        );
        hasher.write(&[argv.len() as u8]);

        Ok(Self {
            id: hasher.finish(),
            argv,
            fn_name: CString::new(fn_name)?,
        })
    }

    pub fn try_from_bytes(input: &[u8]) -> anyhow::Result<CallMeta> {
        if input.len() < Self::MIN_SIZE {
            return Err(anyhow!(
                "provided slice is too short. Expected at least {}",
                Self::MIN_SIZE
            ));
        }

        // Extract ID
        let id = u32::from_ne_bytes([input[0], input[1], input[2], input[3]]);

        // extract argv
        let argc = input[4];
        let min_expected_len = Self::MIN_SIZE + argc as usize;
        if input.len() < min_expected_len {
            return Err(anyhow!(
                "provided slice is too short for argv of length {}. Got {} expected at least {}",
                argc,
                input.len(),
                min_expected_len
            ));
        }
        let argv = if argc > 0 {
            convert_bytes_to_types(
                &input[CallMeta::ARGV_BEGIN..CallMeta::ARGV_BEGIN + argc as usize],
            )?
        } else {
            Vec::new()
        };

        // extract fn_name
        let name_slice = &input[CallMeta::ARGV_BEGIN + argc as usize..];
        if name_slice.last() != Some(&0) {
            return Err(anyhow!("fn_name not null-terminated"));
        }
        let fn_name = CString::from_vec_with_nul(name_slice.to_vec())?;

        Ok(CallMeta { id, argv, fn_name })
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.append(&mut self.id.to_ne_bytes().to_vec());
        bytes.push(self.argv.len() as u8);
        if self.argv.is_empty().not() {
            bytes.extend(self.argv.iter().map(|&dt| dt as u8));
        }
        bytes.extend(self.fn_name.as_bytes_with_nul());

        bytes
    }
}

// Assuming we have a slice of bytes and want to convert to Vec<DataType>
fn convert_bytes_to_types(bytes: &[u8]) -> anyhow::Result<Vec<DataType>> {
    bytes
        .iter()
        .map(|&b| DataType::try_from(b).map_err(|e| anyhow!("Invalid data type value: {}", e)))
        .collect()
}

mod test {
    #![allow(unused)]
    use super::*;

    #[test]
    fn from_bytes() {
        let raw: [u8; 9] = [
            // Hash
            182, 140, 231, 158, // Argc
            0,   // Function name as C string: FOO
            102, 111, 111, 0,
        ];

        let meta = CallMeta::try_from_bytes(raw.as_ref());
        match meta {
            Ok(meta) => {
                assert_eq!(meta.fn_name.to_str().unwrap(), "foo")
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }

    #[test]
    fn to_bytes() {
        let meta = CallMeta::new(vec![DataType::UInt8], "foo").unwrap();

        let raw: [u8; 10] = [
            202, 121, 115, 15, // Hash
            1,  // Argc
            0,  // Argv: UInt8
            102, 111, 111, 0, // Function name as C string: FOO
        ];
        assert_eq!(raw.as_slice(), meta.as_bytes())
    }
}
