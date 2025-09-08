import socket

HOST = "127.0.0.1"   # localhost
PORT = 65433         # use a different port than TCP

with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
    s.bind((HOST, PORT))
    print(f"UDP Server listening on {HOST}:{PORT}")
    while True:
        data, addr = s.recvfrom(1024)
        print(f"Received from {addr}: {data.decode()}")
        s.sendto(data, addr)  # echo the data back
