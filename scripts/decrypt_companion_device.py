#!/usr/bin/env python3
"""
Decode the `encrypt_companion_device` payload produced by the core auth flow.
The logic mirrors `src-tauri/modules/core/src/device/xiaomi/components/auth.rs`.

Example:
    python scripts/decrypt_companion_device.py \\
        --authkey 0123456789abcdeffedcba9876543210 \\
        --phone-random b64:c2FtcGxlcGhvbmVub25jZQ== \\
        --watch-random hex:00112233445566778899aabbccddeeff \\
        --ciphertext BASE64_PAYLOAD

`--phone-random` / `--watch-random` accept either pure hex (default) or a value
prefixed with `hex:` / `b64:` / `base64:`. The ciphertext argument must be the
raw base64 payload copied from the protobuf field.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import hmac
import string
import sys
from typing import Tuple

from cryptography.hazmat.primitives.ciphers.aead import AESCCM

_TAG = b"miwear-auth"


def _parse_authkey(text: str) -> bytes:
    raw = text.strip().lower()
    if raw.startswith("hex:"):
        raw = raw[4:]
    if any(ch not in string.hexdigits for ch in raw) or len(raw) != 32:
        raise argparse.ArgumentTypeError(
            "authkey must be a 16-byte value encoded as 32 hex chars"
        )
    return bytes.fromhex(raw)


def _parse_nonce(value: str, label: str) -> bytes:
    """
    Accept either hex (default) or base64 strings, optionally prefixed.
    """
    raw = value.strip()
    decoder = None
    lowered = raw.lower()
    if lowered.startswith("hex:"):
        raw = raw[4:]
        decoder = "hex"
    elif lowered.startswith("b64:"):
        raw = raw[4:]
        decoder = "b64"
    elif lowered.startswith("base64:"):
        raw = raw[7:]
        decoder = "b64"

    try:
        if decoder == "b64":
            data = base64.b64decode(raw, validate=True)
        elif decoder == "hex" or (
            all(ch in string.hexdigits for ch in raw) and len(raw) == 32
        ):
            data = bytes.fromhex(raw)
        else:
            data = base64.b64decode(raw, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise argparse.ArgumentTypeError(f"failed to parse {label}: {exc}") from exc

    if len(data) != 16:
        raise argparse.ArgumentTypeError(f"{label} must be exactly 16 bytes")
    return data


def _derive_enc_material(
    authkey: bytes, phone_nonce: bytes, watch_nonce: bytes
) -> Tuple[bytes, bytes]:
    init_key = phone_nonce + watch_nonce
    hmac_key = hmac.new(init_key, authkey, hashlib.sha256).digest()

    okm = bytearray(64)
    prev = b""
    offset = 0
    for counter in range(1, 4):
        mac = hmac.new(hmac_key, digestmod=hashlib.sha256)
        mac.update(prev)
        mac.update(_TAG)
        mac.update(bytes([counter]))
        prev = mac.digest()
        take = min(len(prev), 64 - offset)
        okm[offset : offset + take] = prev[:take]
        offset += take
        if offset >= 64:
            break

    enc_key = bytes(okm[16:32])
    enc_nonce = bytes(okm[36:40])
    pkt_nonce = enc_nonce + (b"\x00" * 8)
    return enc_key, pkt_nonce


def _decrypt(enc_key: bytes, nonce: bytes, ciphertext_b64: str) -> bytes:
    try:
        cipher = base64.b64decode(ciphertext_b64.strip(), validate=True)
    except binascii.Error as exc:
        raise ValueError(f"ciphertext is not valid base64: {exc}") from exc

    aes = AESCCM(enc_key, tag_length=4)
    return aes.decrypt(nonce, cipher, None)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Decrypt encrypt_companion_device and print the plaintext in Base64"
    )
    parser.add_argument(
        "--authkey",
        required=True,
        help="16-byte authkey in hex (prefix with hex: if desired)",
    )
    parser.add_argument(
        "--phone-random", required=True, help="phone random (16B) in hex/base64"
    )
    parser.add_argument(
        "--watch-random", required=True, help="watch random (16B) in hex/base64"
    )
    parser.add_argument(
        "--ciphertext",
        required=True,
        help="Base64 blob copied from encrypt_companion_device",
    )
    parser.add_argument(
        "--raw-output",
        action="store_true",
        help="write raw plaintext bytes to stdout instead of Base64 text",
    )
    args = parser.parse_args()

    try:
        authkey = _parse_authkey(args.authkey)
        phone_nonce = _parse_nonce(args.phone_random, "phone-random")
        watch_nonce = _parse_nonce(args.watch_random, "watch-random")
        enc_key, nonce = _derive_enc_material(authkey, phone_nonce, watch_nonce)
        plaintext = _decrypt(enc_key, nonce, args.ciphertext)
    except Exception as exc:  # noqa: BLE001
        print(f"decrypt failed: {exc}", file=sys.stderr)
        sys.exit(1)

    if args.raw_output:
        sys.stdout.buffer.write(plaintext)
    else:
        print(base64.b64encode(plaintext).decode("ascii"))


if __name__ == "__main__":
    main()
