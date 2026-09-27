"""Shared binary fixtures for Python distribution tests."""


def pe(machine: int, offset: int = 0x80) -> bytes:
    image = bytearray(offset + 8)
    image[:2] = b'MZ'
    image[60:64] = offset.to_bytes(4, 'little')
    image[offset:offset + 4] = b'PE\0\0'
    image[offset + 4:offset + 6] = machine.to_bytes(2, 'little')
    return bytes(image)
