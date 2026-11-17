#!/usr/bin/env python3
"""Verify DoH GET/POST, A/AAAA/HTTPS/NXDOMAIN and malformed-request rejection."""
import base64
import struct
import sys
import urllib.error
import urllib.request


def query(host, kind, transaction):
    labels = b"".join(bytes([len(label)]) + label.encode("ascii") for label in host.split("."))
    return struct.pack("!6H", transaction, 0x0100, 1, 0, 0, 0) + labels + b"\0" + struct.pack("!2H", kind, 1)


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


def verify(packet, request, kind, expected_rcode):
    assert len(packet) >= len(request), "truncated DNS response"
    transaction, flags, questions, answers, authorities, additional = struct.unpack("!6H", packet[:12])
    assert transaction == struct.unpack("!H", request[:2])[0] and flags & 0x8000, "DNS transaction mismatch"
    assert flags & 0x0200 == 0 and flags & 15 == expected_rcode, "DNS error or truncated response"
    assert questions == 1 and packet[12:len(request)] == request[12:], "DNS question mismatch"
    offset = len(request)
    matches = 0
    for index in range(answers + authorities + additional):
        offset = name_end(packet, offset)
        record_kind, cls, _ttl, size = struct.unpack("!HHIH", packet[offset:offset + 10])
        offset += 10
        data = packet[offset:offset + size]
        assert len(data) == size, "truncated DNS record"
        if index < answers and record_kind == kind and cls == 1:
            assert (kind == 1 and size == 4) or (kind == 28 and size == 16) or (kind == 65 and size >= 3), "invalid DNS record size"
            matches += 1
        offset += size
    assert offset == len(packet), "trailing DNS data"
    assert expected_rcode == 3 or matches > 0, "missing requested answer type"
    return matches


def main():
    endpoint = sys.argv[1].rstrip("/") + "/dns-query"
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    cases = [("example.com", 1, 0, "A"), ("example.com", 28, 0, "AAAA"),
             ("cloudflare.com", 65, 0, "HTTPS"), ("mirrorproxy-probe.invalid", 1, 3, "NXDOMAIN")]
    for method in ("GET", "POST"):
        for index, (host, kind, rcode, label) in enumerate(cases):
            wire = query(host, kind, 0x1234 + index + (16 if method == "POST" else 0))
            url = endpoint
            body = wire if method == "POST" else None
            if method == "GET":
                url += "?dns=" + base64.urlsafe_b64encode(wire).rstrip(b"=").decode()
            request = urllib.request.Request(url, data=body, method=method, headers={
                "Accept": "application/dns-message", "Content-Type": "application/dns-message",
            })
            with opener.open(request, timeout=10) as response:
                assert response.status == 200
                assert response.headers.get_content_type() == "application/dns-message"
                assert response.headers.get("Cache-Control") == "no-store"
                packet = response.read(65536)
                assert len(packet) <= 65535
                matches = verify(packet, wire, kind, rcode)
                print(f"{method}: verified {host} {label}, RCODE={rcode}, matching answers={matches}")
    try:
        opener.open(endpoint + "?dns=invalid!", timeout=10)
    except urllib.error.HTTPError as error:
        assert error.code == 400, f"expected 400, got {error.code}"
    else:
        raise AssertionError("malformed DNS request was accepted")
    print("Malformed request: rejected")


if __name__ == "__main__":
    main()
