import socket

HOST = "127.0.0.1"   # server hostname or IP
PORT = 65433         # same as the server

with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
    message = b"Hello UDP Protocol World"
    s.sendto(message, (HOST, PORT))
    data, server = s.recvfrom(1024)

print(f"Received back: {data.decode()}")

