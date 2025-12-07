use sha2::{Digest, Sha256};
use std::io::{self, Error, ErrorKind};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Store = 0x01,
    Retrieve = 0x02,
    Delete = 0x03,
    List = 0x04,
    Auth = 0x05, // New: authentication
}

impl Operation {
    pub fn from_u8(value: u8) -> io::Result<Self> {
        match value {
            0x01 => Ok(Operation::Store),
            0x02 => Ok(Operation::Retrieve),
            0x03 => Ok(Operation::Delete),
            0x04 => Ok(Operation::List),
            0x05 => Ok(Operation::Auth),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid operation code")),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusCode {
    Success = 0x00,
    ErrorNotFound = 0x01,
    ErrorPermissionDenied = 0x02,
    ErrorInvalidData = 0x03,
    ErrorServerError = 0x04,
}

impl StatusCode {
    pub fn from_u8(value: u8) -> io::Result<Self> {
        match value {
            0x00 => Ok(StatusCode::Success),
            0x01 => Ok(StatusCode::ErrorNotFound),
            0x02 => Ok(StatusCode::ErrorPermissionDenied),
            0x03 => Ok(StatusCode::ErrorInvalidData),
            0x04 => Ok(StatusCode::ErrorServerError),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid status code")),
        }
    }
}

#[derive(Debug)]
pub struct Message {
    pub request_id: u32,
    pub operation: Operation,
    pub status: StatusCode,
    pub checksum: [u8; 32],
    pub auth_token: [u8; 32],
    pub payload: Vec<u8>,
}

impl Message {
    pub fn new(operation: Operation, payload: Vec<u8>) -> Self {
        let checksum = Self::calculate_checksum(&payload);
        Self {
            request_id: 0,
            operation,
            status: StatusCode::Success,
            checksum,
            auth_token: [0u8; 32],
            payload,
        }
    }

    pub fn new_with_auth(operation: Operation, payload: Vec<u8>, auth_token: [u8; 32]) -> Self {
        let checksum = Self::calculate_checksum(&payload);
        Self {
            request_id: 0,
            operation,
            status: StatusCode::Success,
            checksum,
            auth_token,
            payload,
        }
    }

    pub fn new_response(
        request_id: u32,
        operation: Operation,
        status: StatusCode,
        payload: Vec<u8>,
    ) -> Self {
        let checksum = Self::calculate_checksum(&payload);
        Self {
            request_id,
            operation,
            status,
            checksum,
            auth_token: [0u8; 32],
            payload,
        }
    }

    pub fn set_request_id(&mut self, id: u32) {
        self.request_id = id;
    }

    fn calculate_checksum(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    pub fn verify_checksum(&self) -> bool {
        let calculated = Self::calculate_checksum(&self.payload);
        calculated == self.checksum
    }

    /// Serialize message to bytes
    /// Format: [length: u32][request_id: u32][op: u8][status: u8][checksum: 32][auth: 32][payload]
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;
        let total_len = 4 + 1 + 1 + 32 + 32 + payload_len; // request_id + op + status + checksum + auth + payload

        let mut bytes = Vec::with_capacity(4 + total_len as usize);

        // Length prefix
        bytes.extend_from_slice(&total_len.to_be_bytes());

        // Request ID
        bytes.extend_from_slice(&self.request_id.to_be_bytes());

        // Operation
        bytes.push(self.operation as u8);

        // Status
        bytes.push(self.status as u8);

        // Checksum
        bytes.extend_from_slice(&self.checksum);

        // Auth token
        bytes.extend_from_slice(&self.auth_token);

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Parse message from bytes
    pub fn from_bytes(length: u32, data: &[u8]) -> io::Result<Self> {
        if data.len() < 70 {
            // Minimum: 4 + 1 + 1 + 32 + 32
            return Err(Error::new(ErrorKind::InvalidData, "Message too short"));
        }

        let mut offset = 0;

        // Request ID
        let request_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        offset += 4;

        // Operation
        let operation = Operation::from_u8(data[offset])?;
        offset += 1;

        // Status
        let status = StatusCode::from_u8(data[offset])?;
        offset += 1;

        // Checksum
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;

        // Auth token
        let mut auth_token = [0u8; 32];
        auth_token.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;

        // Payload
        let payload = data[offset..].to_vec();

        if payload.len() != (length as usize - 70) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Payload length mismatch",
            ));
        }

        let msg = Self {
            request_id,
            operation,
            status,
            checksum,
            auth_token,
            payload,
        };

        // Verify checksum
        if !msg.verify_checksum() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Checksum verification failed",
            ));
        }

        Ok(msg)
    }
}

// Simple authentication helper
pub fn generate_auth_token(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_verification() {
        let msg = Message::new(Operation::Store, b"test data".to_vec());
        assert!(msg.verify_checksum());
    }

    #[test]
    fn test_message_with_auth() {
        let token = generate_auth_token("my_secret_password");
        let msg = Message::new_with_auth(Operation::Store, b"data".to_vec(), token);
        assert_eq!(msg.auth_token, token);
    }

    #[test]
    fn test_enhanced_serialization() {
        let mut msg = Message::new(Operation::Store, b"test".to_vec());
        msg.set_request_id(42);

        let bytes = msg.to_bytes();
        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let parsed = Message::from_bytes(length, &bytes[4..]).unwrap();

        assert_eq!(parsed.request_id, 42);
        assert_eq!(parsed.operation, Operation::Store);
        assert_eq!(parsed.payload, b"test");
        assert!(parsed.verify_checksum());
    }
}
