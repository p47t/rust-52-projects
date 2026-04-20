use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("unexpected end of file reading {bytes} bytes (offset: {offset}, len: {len})")]
    UnexpectedEof {
        bytes: usize,
        offset: usize,
        len: usize,
    },
    #[error("invalid GGUF magic bytes: expected 0x46554747 ('GGUF'), found 0x{0:08X}")]
    InvalidMagic(u32),
    #[error("unsupported GGUF version: {0}")]
    UnsupportedVersion(u32),
    #[error("unknown value type ID {0}")]
    UnknownValueType(u32),
    #[error("unknown GGML type ID {0}")]
    UnknownGgmlType(u32),
    #[error("invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("slice conversion failed: {0}")]
    TryFromSlice(#[from] std::array::TryFromSliceError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ValueType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl TryFrom<u32> for ValueType {
    type Error = ParserError;

    fn try_from(val: u32) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Uint8),
            1 => Ok(Self::Int8),
            2 => Ok(Self::Uint16),
            3 => Ok(Self::Int16),
            4 => Ok(Self::Uint32),
            5 => Ok(Self::Int32),
            6 => Ok(Self::Float32),
            7 => Ok(Self::Bool),
            8 => Ok(Self::String),
            9 => Ok(Self::Array),
            10 => Ok(Self::Uint64),
            11 => Ok(Self::Int64),
            12 => Ok(Self::Float64),
            _ => Err(ParserError::UnknownValueType(val)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
#[repr(u32)]
pub enum GgmlType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2_K = 10,
    Q3_K = 11,
    Q4_K = 12,
    Q5_K = 13,
    Q6_K = 14,
    Q8_K = 15,
    Iq2Xxs = 16,
    Iq2Xs = 17,
    Iq3Xxs = 18,
    Iq1S = 19,
    Iq4Nl = 20,
    Iq3S = 21,
    Iq2S = 22,
    Iq4Xs = 23,
    I8 = 24,
    I16 = 25,
    I32 = 26,
    I64 = 27,
    F64 = 28,
    Iq1M = 29,
}

impl TryFrom<u32> for GgmlType {
    type Error = ParserError;

    fn try_from(val: u32) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            9 => Ok(Self::Q8_1),
            10 => Ok(Self::Q2_K),
            11 => Ok(Self::Q3_K),
            12 => Ok(Self::Q4_K),
            13 => Ok(Self::Q5_K),
            14 => Ok(Self::Q6_K),
            15 => Ok(Self::Q8_K),
            16 => Ok(Self::Iq2Xxs),
            17 => Ok(Self::Iq2Xs),
            18 => Ok(Self::Iq3Xxs),
            19 => Ok(Self::Iq1S),
            20 => Ok(Self::Iq4Nl),
            21 => Ok(Self::Iq3S),
            22 => Ok(Self::Iq2S),
            23 => Ok(Self::Iq4Xs),
            24 => Ok(Self::I8),
            25 => Ok(Self::I16),
            26 => Ok(Self::I32),
            27 => Ok(Self::I64),
            28 => Ok(Self::F64),
            29 => Ok(Self::Iq1M),
            _ => Err(ParserError::UnknownGgmlType(val)),
        }
    }
}

impl fmt::Display for GgmlType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::F32 => "F32",
            Self::F16 => "F16",
            Self::Q4_0 => "Q4_0",
            Self::Q4_1 => "Q4_1",
            Self::Q5_0 => "Q5_0",
            Self::Q5_1 => "Q5_1",
            Self::Q8_0 => "Q8_0",
            Self::Q8_1 => "Q8_1",
            Self::Q2_K => "Q2_K",
            Self::Q3_K => "Q3_K",
            Self::Q4_K => "Q4_K",
            Self::Q5_K => "Q5_K",
            Self::Q6_K => "Q6_K",
            Self::Q8_K => "Q8_K",
            Self::Iq2Xxs => "IQ2_XXS",
            Self::Iq2Xs => "IQ2_XS",
            Self::Iq3Xxs => "IQ3_XXS",
            Self::Iq1S => "IQ1_S",
            Self::Iq4Nl => "IQ4_NL",
            Self::Iq3S => "IQ3_S",
            Self::Iq2S => "IQ2_S",
            Self::Iq4Xs => "IQ4_XS",
            Self::I8 => "I8",
            Self::I16 => "I16",
            Self::I32 => "I32",
            Self::I64 => "I64",
            Self::F64 => "F64",
            Self::Iq1M => "IQ1_M",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Array(ValueType, Vec<Value>),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uint8(v) => write!(f, "{}", v),
            Self::Int8(v) => write!(f, "{}", v),
            Self::Uint16(v) => write!(f, "{}", v),
            Self::Int16(v) => write!(f, "{}", v),
            Self::Uint32(v) => write!(f, "{}", v),
            Self::Int32(v) => write!(f, "{}", v),
            Self::Float32(v) => write!(f, "{}", v),
            Self::Bool(v) => write!(f, "{}", v),
            Self::String(v) => write!(f, "\"{}\"", v),
            Self::Uint64(v) => write!(f, "{}", v),
            Self::Int64(v) => write!(f, "{}", v),
            Self::Float64(v) => write!(f, "{}", v),
            Self::Array(t, v) => {
                write!(f, "[Type: {:?}, Len: {}, Elements: [", t, v.len())?;
                for (i, elem) in v.iter().take(5).enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                if v.len() > 5 {
                    write!(f, ", ... +{} more", v.len() - 5)?;
                }
                write!(f, "]]")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TensorInfo {
    pub name: String,
    pub dimensions: Vec<u64>,
    pub tensor_type: GgmlType,
    pub offset: u64,
}

#[derive(Debug)]
pub struct GgufFile {
    pub version: u32,
    pub metadata: HashMap<String, Value>,
    pub tensors: Vec<TensorInfo>,
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ParserError> {
        let slice =
            self.data
                .get(self.offset..self.offset + len)
                .ok_or(ParserError::UnexpectedEof {
                    bytes: len,
                    offset: self.offset,
                    len: self.data.len(),
                })?;
        self.offset += len;
        Ok(slice)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], ParserError> {
        let slice =
            self.data
                .get(self.offset..self.offset + N)
                .ok_or(ParserError::UnexpectedEof {
                    bytes: N,
                    offset: self.offset,
                    len: self.data.len(),
                })?;
        self.offset += N;
        Ok(slice.try_into()?)
    }

    fn read_u8(&mut self) -> Result<u8, ParserError> {
        let array = self.read_array::<1>()?;
        Ok(array[0])
    }

    fn read_u32(&mut self) -> Result<u32, ParserError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, ParserError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_i8(&mut self) -> Result<i8, ParserError> {
        Ok(self.read_u8()? as i8)
    }

    fn read_i16(&mut self) -> Result<i16, ParserError> {
        Ok(i16::from_le_bytes(self.read_array()?))
    }

    fn read_u16(&mut self) -> Result<u16, ParserError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_i32(&mut self) -> Result<i32, ParserError> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }

    fn read_i64(&mut self) -> Result<i64, ParserError> {
        Ok(i64::from_le_bytes(self.read_array()?))
    }

    fn read_f32(&mut self) -> Result<f32, ParserError> {
        Ok(f32::from_le_bytes(self.read_array()?))
    }

    fn read_f64(&mut self) -> Result<f64, ParserError> {
        Ok(f64::from_le_bytes(self.read_array()?))
    }

    fn read_string(&mut self) -> Result<String, ParserError> {
        let len = self.read_u64()? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(ParserError::from)
    }

    fn read_value(&mut self, val_type: ValueType) -> Result<Value, ParserError> {
        match val_type {
            ValueType::Uint8 => Ok(Value::Uint8(self.read_u8()?)),
            ValueType::Int8 => Ok(Value::Int8(self.read_i8()?)),
            ValueType::Uint16 => Ok(Value::Uint16(self.read_u16()?)),
            ValueType::Int16 => Ok(Value::Int16(self.read_i16()?)),
            ValueType::Uint32 => Ok(Value::Uint32(self.read_u32()?)),
            ValueType::Int32 => Ok(Value::Int32(self.read_i32()?)),
            ValueType::Float32 => Ok(Value::Float32(self.read_f32()?)),
            ValueType::Bool => Ok(Value::Bool(self.read_u8()? != 0)),
            ValueType::String => Ok(Value::String(self.read_string()?)),
            ValueType::Uint64 => Ok(Value::Uint64(self.read_u64()?)),
            ValueType::Int64 => Ok(Value::Int64(self.read_i64()?)),
            ValueType::Float64 => Ok(Value::Float64(self.read_f64()?)),
            ValueType::Array => {
                let elem_type_u32 = self.read_u32()?;
                let elem_type = ValueType::try_from(elem_type_u32)?;
                let len = self.read_u64()? as usize;
                let mut elements = Vec::with_capacity(len);
                for _ in 0..len {
                    elements.push(self.read_value(elem_type)?);
                }
                Ok(Value::Array(elem_type, elements))
            }
        }
    }
}

pub fn parse_gguf(data: impl AsRef<[u8]>) -> Result<GgufFile, ParserError> {
    let mut reader = Reader::new(data.as_ref());

    // 1. Parse Magic
    let magic = reader.read_u32()?;
    if magic != 0x46554747 {
        return Err(ParserError::InvalidMagic(magic));
    }

    // 2. Parse Version
    let version = reader.read_u32()?;
    if version != 1 && version != 2 && version != 3 {
        return Err(ParserError::UnsupportedVersion(version));
    }

    // 3. Parse counts (depends on version)
    let (tensor_count, metadata_kv_count) = if version == 1 {
        let t_count = reader.read_u32()? as u64;
        let m_count = reader.read_u32()? as u64;
        (t_count, m_count)
    } else {
        let t_count = reader.read_u64()?;
        let m_count = reader.read_u64()?;
        (t_count, m_count)
    };

    // 4. Parse Metadata Key-Value pairs
    let mut metadata = HashMap::with_capacity(metadata_kv_count as usize);
    for _ in 0..metadata_kv_count {
        let key = reader.read_string()?;
        let val_type_u32 = reader.read_u32()?;
        let val_type = ValueType::try_from(val_type_u32)?;
        let val = reader.read_value(val_type)?;
        metadata.insert(key, val);
    }

    // 5. Parse Tensor Info
    let mut tensors = Vec::with_capacity(tensor_count as usize);
    for _ in 0..tensor_count {
        let name = reader.read_string()?;
        let dimensions_count = reader.read_u32()? as usize;
        let mut dimensions = Vec::with_capacity(dimensions_count);
        for _ in 0..dimensions_count {
            dimensions.push(reader.read_u64()?);
        }
        let tensor_type_u32 = reader.read_u32()?;
        let tensor_type = GgmlType::try_from(tensor_type_u32)?;
        let offset = reader.read_u64()?;

        tensors.push(TensorInfo {
            name,
            dimensions,
            tensor_type,
            offset,
        });
    }

    Ok(GgufFile {
        version,
        metadata,
        tensors,
    })
}
