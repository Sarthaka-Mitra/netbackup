use std::io::{self, Error, ErrorKind};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Store = 0x01,
    Retrieve = 0x02,
    Delete = 0x03,
    List = 0x04,
}

impl Operation {
    pub fn from_u8(value: u8) -> io::Result<Self> {
        match value {
            0x01 => Ok(Operation::Store),
            0x02 => Ok(Operation::Retrieve),
            0x03 => Ok(Operation::Delete),
            0x04 => Ok(Operation::List),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid operation code")),
        }
    }
}

#[derive(Debug)]
pub struct Message {
    pub operation: Operation,
    pub payload: Vec<u8>,
}

impl Message {
    pub fn new(operation: Operation, payload: Vec<u8>) -> Self {
        Self { operation, payload }
    }

    // Serialise message to bytes: [length: u32][op: u8][payload]
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;
        let total_len = payload_len + 1;

        let mut bytes = Vec::with_capacity(4 + 1 + self.payload.len());
        bytes.extend_from_slice(&total_len.to_be_bytes());
        bytes.push(self.operation as u8);
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    //Parse message from bytes
    pub fn from_bytes(length: u32, data: &[u8]) -> io::Result<Self> {
        if data.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "Empty message data"));
        }

        let operation = Operation::from_u8(data[0])?;
        let payload = data[1..].to_vec();

        if payload.len() != (length as usize - 1) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Payload length mismatch",
            ));
        }

        Ok(Self { operation, payload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::new(Operation::Store, b"test data".to_vec());
        let bytes = msg.to_bytes();

        // Check length prefix (4 bytes)
        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(length, 10); //1 (op) + 9(payload)

        // Check operation
        assert_eq!(bytes[4], Operation::Store as u8);

        // Check payload
        assert_eq!(&bytes[5..], b"test data");
    }

    #[test]
    fn test_message_deserialization() {
        let original = Message::new(Operation::Retrieve, b"filename.txt".to_vec());
        let bytes = original.to_bytes();

        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let parsed = Message::from_bytes(length, &bytes[4..]).unwrap();

        assert_eq!(parsed.operation, Operation::Retrieve);
        assert_eq!(parsed.payload, b"filename.txt");
    }
}
