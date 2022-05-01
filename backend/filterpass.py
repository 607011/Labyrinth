#!/usr/bin/env python3

import hashlib
import struct

def passwords():
    for line in open('toppass.txt', 'r'):
        password = line.strip()
        if len(password) < 8:
            continue
        hash = hashlib.md5(password.encode('utf-8')).digest()
        yield hash

def main():
    hashes = [hash for hash in passwords()]
    hashes.sort()
    with open('toppass8-md5.bin', 'wb') as f:
        for hash in hashes:
            f.write(hash)

if __name__ == '__main__':
    main()