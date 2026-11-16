#!/usr/bin/env python3
"""Validate a deployed DoH route using only the Python standard library."""
import base64
import struct
import sys
import urllib.error
import urllib.request

QUERY = bytes.fromhex("123401000001000000000000") + b"\x07example\x03com\0\0\x01\0\x01"


def name_end(packet, offset):
    for _ in range(128):
        size = packet[offset]
        if size & 0xC0 == 0xC0:
            assert offset + 2 <= len(packet), "truncated compression pointer"
            return offset + 2
        assert size <= 63, "invalid DNS label"
        offset += size + 1
        assert offset <= len(packet), "truncated DNS label"
        if size == 0:
            return offset
    raise AssertionError("DNS name too long")


def verify(packet):
    assert len(packet) >= len(QUERY), "truncated DNS response"
    transaction, flags, questions, answers, authorities, additional = struct.unpack("!6H", packet[:12])
    assert transaction == 0x1234 and flags & 0x8000, "DNS transaction mismatch"
    assert flags & 0x020F == 0, "DNS error or truncated response"
    assert questions == 1 and answers > 0, "missing DNS answer"
    assert packet[12:len(QUERY)] == QUERY[12:], "DNS question mismatch"
    offset = len(QUERY)
    addresses = []
    for index in range(answers + authorities + additional):
        offset = name_end(packet, offset)
        kind, cls, _ttl, size = struct.unpack("!HHIH", packet[offset:offset + 10])
        offset += 10
        data = packet[offset:offset + size]
        assert len(data) == size, "truncated DNS record"
        if index < answers and kind == 1 and cls == 1:
            assert size == 4, "invalid A record"
            addresses.append(".".join(map(str, data)))
        offset += size
    assert offset == len(packet) and addresses, "missing A record or trailing data"
    return addresses


def main():
    endpoint = sys.argv[1].rstrip("/") + "/dns-query"
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    for method in ("GET", "POST"):
        url = endpoint
        body = QUERY if method == "POST" else None
        if method == "GET":
            url += "?dns=" + base64.urlsafe_b64encode(QUERY).rstrip(b"=").decode()
        request = urllib.request.Request(url, data=body, method=method, headers={
            "Accept": "application/dns-message", "Content-Type": "application/dns-message",
        })
        with opener.open(request, timeout=65) as response:
            assert response.status == 200
            assert response.headers.get_content_type() == "application/dns-message"
            assert response.headers.get("Cache-Control") == "no-store"
            packet = response.read(65536)
            assert len(packet) <= 65535
            print(f"{method}: verified example.com A {', '.join(verify(packet))}")
    try:
        opener.open(endpoint + "?dns=invalid!", timeout=10)
    except urllib.error.HTTPError as error:
        assert error.code == 400, f"expected 400, got {error.code}"
    else:
        raise AssertionError("malformed DNS request was accepted")
    print("Malformed request: rejected")


if __name__ == "__main__":
    main()
