import socket
import subprocess
import re
import sys
import ipaddress
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime

PORT_MAP = {
    80: "HTTP",
    443: "HTTPS",
    23: "Telnet",
    22: "SSH",
    554: "RTSP",
    8000: "Dahua/Alt",
    7000: "Hikvision",
    37777: "Dahua",
    34567: "Hikvision",
    8080: "HTTP-Alt",
    161: "SNMP",
    1900: "UPnP/SSDP",
    8554: "RTSP-Alt",
    5000: "UPnP",
    3702: "WS-Discovery",
    5353: "mDNS",
}

VENDOR_PREFIXES = [
    ("00:1b:1c", "Hikvision", "camera/nvr"),
    ("00:10:1e", "Tatung", "camera"),
    ("00:0c:e7", "Vivotek", "camera"),
    ("00:02:d1", "Arecont", "camera"),
    ("00:12:4b", "Mobotix", "camera"),
    ("00:18:9e", "D-Link", "camera/switch"),
    ("00:01:c8", "Panasonic", "camera"),
    ("00:0f:61", "Samsung", "camera"),
    ("00:1a:4f", "Axis", "camera"),
    ("00:08:5c", "Ubiquiti", "switch/camera"),
    ("68:d7:9a", "TP-Link", "switch"),
    ("a0:f3:e4", "TP-Link", "switch"),
    ("00:06:5b", "Netgear", "switch"),
    ("00:14:bf", "Cisco", "switch"),
    ("00:1a:6c", "Cisco", "switch"),
    ("00:0c:85", "HP/Aruba", "switch"),
    ("00:17:c6", "HP/3Com", "switch"),
    ("00:1f:33", "Huawei", "switch"),
    ("00:18:82", "ZTE", "switch"),
    ("00:1b:8f", "MikroTik", "switch"),
    ("4c:5e:0c", "MikroTik", "switch"),
    ("00:0f:e2", "Dahua", "camera/nvr"),
    ("a8:4a:05", "Dahua", "camera/nvr"),
    ("00:1e:3a", "Huawei", "switch"),
    ("00:25:53", "Cisco", "switch"),
    ("00:1d:a1", "Alcatel", "switch"),
    ("00:0b:5f", "Juniper", "switch"),
    ("00:1b:d4", "Dell", "switch"),
    ("00:15:5d", "HP", "switch"),
    ("00:26:55", "Buffalo", "switch"),
    ("00:22:75", "DrayTek", "switch"),
    ("00:0c:29", "VMware", "virtual"),
    ("00:50:56", "VMware", "virtual"),
    ("00:1c:14", "VMware", "virtual"),
    ("00:25:90", "Synology", "NAS"),
    ("00:11:32", "Synology", "NAS"),
    ("00:1b:21", "QNAP", "NAS"),
    ("00:0e:58", "Intel", "generic"),
    ("00:1b:fc", "Intel", "generic"),
    ("00:15:17", "Apple", "generic"),
    ("ec:35:86", "Apple", "generic"),
    ("b8:27:eb", "Raspberry Pi", "generic"),
    ("dc:a6:32", "Raspberry Pi", "generic"),
    ("00:13:95", "Grandstream", "voip/camera"),
    ("00:0b:82", "Grandstream", "voip"),
    ("ec:17:2f", "HP", "generic"),
    ("3c:d9:2b", "HP", "generic"),
    ("00:1b:63", "Intel", "generic"),
    ("00:1e:67", "Intel", "generic"),
    ("3c:52:82", "Intel", "generic"),
    ("98:90:96", "ASUS", "switch/router"),
    ("00:1a:92", "ASUS", "switch/router"),
    ("1c:87:2c", "Micro-Star", "generic"),
    ("00:e0:4c", "Realtek", "generic"),
    ("00:1a:a0", "D-Link", "switch/router"),
    ("b0:48:7a", "D-Link", "switch/router"),
    ("00:1a:6b", "Zyxel", "switch"),
    ("d8:5d:e2", "Zyxel", "switch"),
    ("00:0f:b5", "3Com", "switch"),
    ("00:1c:58", "Trendnet", "switch/camera"),
    ("00:07:c9", "Edimax", "camera/switch"),
    ("48:22:54", "Edimax", "camera/switch"),
    ("00:18:fe", "Liteon", "generic"),
    ("00:40:63", "Allied Telesis", "switch"),
    ("00:80:5f", "Lantronix", "generic"),
    ("00:22:6b", "MSI", "generic"),
    ("00:18:34", "Gigabyte", "generic"),
    ("00:1b:24", "ASRock", "generic"),
    ("00:0c:6e", "AVM", "router"),
    ("00:1a:79", "AVM", "router"),
    ("0c:37:dc", "Hikvision", "camera/nvr"),
    ("44:19:b6", "Hikvision", "camera/nvr"),
    ("8c:ea:1b", "Hikvision", "camera/nvr"),
    ("3c:e3:6b", "Hikvision", "camera/nvr"),
    ("bc:ad:28", "Hikvision", "camera/nvr"),
    ("24:7e:12", "Uniview", "camera/nvr"),
    ("00:04:25", "Panasonic i-Pro", "camera"),
    ("00:12:6b", "Toshiba", "camera"),
    ("00:07:5f", "Sony", "camera"),
    ("00:11:2f", "JVC", "camera"),
    ("00:18:70", "ACTi", "camera"),
    ("00:0c:91", "Arecont Vision", "camera"),
    ("00:1d:92", "GeoVision", "camera/nvr"),
    ("00:01:af", "Bosch", "camera"),
    ("00:0a:ba", "Bosch", "camera"),
    ("00:17:31", "EverFocus", "camera"),
    ("00:04:0f", "Honeywell", "camera"),
    ("5c:0e:8b", "Honeywell", "camera"),
    ("00:0d:6e", "Hikvision", "camera/nvr"),
]

TIMEOUT = 2
MAX_WORKERS = 100

def get_local_cidr():
    try:
        out = subprocess.check_output(["ipconfig"], text=True, creationflags=subprocess.CREATE_NO_WINDOW)
        ips = re.findall(r"IPv4[^:]*:\s*(\d+\.\d+\.\d+\.\d+)", out)
        masks = re.findall(r"Subnet[^:]*:\s*(\d+\.\d+\.\d+\.\d+)", out)
        for ip, mask in zip(ips, masks):
            if ip.startswith("127.") or ip.startswith("169."):
                continue
            m = sum(bin(int(x)).count("1") for x in mask.split("."))
            return f"{ip}/{m}"
        return None
    except Exception as e:
        print(f"[!] Gagal detect network: {e}")
        return None

def ping_host(ip):
    try:
        out = subprocess.check_output(
            ["ping", "-n", "1", "-w", str(TIMEOUT * 1000), ip],
            stderr=subprocess.DEVNULL, text=True, creationflags=subprocess.CREATE_NO_WINDOW
        )
        return "TTL=" in out or "Reply from" in out
    except:
        return False

def scan_port(ip, port):
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(TIMEOUT)
        result = s.connect_ex((ip, port))
        s.close()
        return result == 0
    except:
        return False

def get_banner(ip, port):
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(3)
        s.connect((ip, port))
        if port in (80, 443, 8080, 8000):
            s.sendall(b"GET / HTTP/1.0\r\nHost: " + ip.encode() + b"\r\n\r\n")
            data = s.recv(1024).decode("utf-8", errors="ignore")
            s.close()
            if "Server:" in data:
                m = re.search(r"Server:\s*(.+?)\r\n", data, re.IGNORECASE)
                if m: return m.group(1).strip()
            return data[:100].strip()
        s.close()
        return ""
    except:
        return ""

def get_mac_vendor(mac):
    mac_clean = mac.upper().replace("-", ":").replace(".", ":")
    for prefix, vendor, dtype in VENDOR_PREFIXES:
        if mac_clean.startswith(prefix.upper()):
            return vendor, dtype
    return None, None

def guess_device_type(ip, mac, ports, banner_info):
    vendor, dtype_vendor = get_mac_vendor(mac)
    guessed = set()
    if dtype_vendor:
        guessed.add(dtype_vendor)

    if 554 in ports or 8554 in ports:
        guessed.add("camera")
    if 8000 in ports or 37777 in ports:
        guessed.add("nvr")
    if 34567 in ports or 7000 in ports:
        guessed.add("nvr")
    if 23 in ports:
        guessed.add("switch")  # many switches have telnet
    if 161 in ports:
        guessed.add("switch")  # SNMP

    banner_lower = banner_info.lower()
    if "hikvision" in banner_lower:
        guessed.add("camera")
        guessed.add("nvr")
    if "dahua" in banner_lower:
        guessed.add("camera")
        guessed.add("nvr")
    if "axis" in banner_lower:
        guessed.add("camera")
    if "dvr" in banner_lower or "nvr" in banner_lower:
        guessed.add("nvr")
    if "switch" in banner_lower or "smart" in banner_lower:
        guessed.add("switch")
    if "broadcast" in banner_lower or "public address" in banner_lower:
        guessed.add("broadcast")

    if vendor in ("Cisco", "HP/Aruba", "TP-Link", "Netgear", "Huawei", "Juniper", "MikroTik", "Zyxel", "D-Link"):
        guessed.add("switch")
    if vendor in ("Hikvision", "Dahua", "Axis", "Vivotek", "Panasonic", "Uniview"):
        guessed.add("camera")
    if vendor in ("Synology", "QNAP"):
        guessed.add("nvr")

    if not guessed:
        if ports:
            guessed.add("device")
        else:
            guessed.add("unknown")
    return vendor, guessed

def scan_network(network_str):
    print(f"\n{'='*60}")
    print(f"Network Scanner — {datetime.now().strftime('%H:%M:%S')}")
    print(f"Scanning: {network_str}")
    print(f"{'='*60}\n")

    network = ipaddress.ip_network(network_str, strict=False)
    hosts = [str(ip) for ip in network.hosts()]

    print(f"[*] Ping sweep {len(hosts)} hosts...")
    active = []
    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as ex:
        futs = {ex.submit(ping_host, ip): ip for ip in hosts}
        for fut in as_completed(futs):
            ip = futs[fut]
            if fut.result():
                active.append(ip)
                sys.stdout.write(f"\r  [+] Active: {len(active)}")
                sys.stdout.flush()
    print(f"\n[*] Found {len(active)} active hosts\n")

    if not active:
        print("[!] No active hosts found. Coba jalankan sebagai Administrator.")
        return

    # Build ARP table
    try:
        out = subprocess.check_output(["arp", "-a"], text=True, creationflags=subprocess.CREATE_NO_WINDOW)
        arp_lines = out.splitlines()
    except:
        arp_lines = []

    arp_table = {}
    for line in arp_lines:
        m = re.search(r"(\d+\.\d+\.\d+\.\d+)\s+([0-9a-fA-F-]{17})", line)
        if m:
            arp_table[m.group(1)] = m.group(2)

    results = {}
    print(f"[*] Scanning ports on {len(active)} hosts...")
    CHECK_PORTS = list(PORT_MAP.keys())
    for idx, ip in enumerate(active):
        mac = arp_table.get(ip, "?")
        open_ports = []
        with ThreadPoolExecutor(max_workers=20) as ex:
            futs = {ex.submit(scan_port, ip, p): p for p in CHECK_PORTS}
            for fut in as_completed(futs):
                port = futs[fut]
                if fut.result():
                    open_ports.append(port)
        open_ports.sort()
        banner = ""
        for p in open_ports:
            if p in (80, 443, 8000, 8080):
                banner = get_banner(ip, p)
                if banner:
                    break
        vendor, types = guess_device_type(ip, mac, open_ports, banner)
        results[ip] = {
            "mac": mac, "ports": open_ports,
            "vendor": vendor, "types": types,
            "banner": banner
        }
        sys.stdout.write(f"\r  [{idx+1}/{len(active)}] {ip}")
        sys.stdout.flush()
    print()

    # Group by type
    categories = {
        "camera": [], "nvr": [], "switch": [],
        "broadcast": [], "device": [], "unknown": []
    }
    for ip, info in sorted(results.items(), key=lambda x: x[0]):
        for t in info["types"]:
            if t in categories:
                categories[t].append(ip)
                break
        else:
            categories["unknown"].append(ip)

    def print_group(title, ips, color=""):
        if not ips:
            return
        print(f"\n  {title} ({len(ips)})")
        for ip in sorted(ips):
            i = results[ip]
            port_str = ", ".join(f"{p}" for p in i["ports"]) if i["ports"] else "-"
            banner_short = i["banner"][:60] if i["banner"] else ""
            vendor_str = f" [{i['vendor']}]" if i["vendor"] else ""
            mac_str = i["mac"] if i["mac"] != "?" else ""
            print(f"    {ip:15}  {mac_str:18}{vendor_str}  [{port_str}]  {banner_short}")

    print_group("CAMERA", categories["camera"])
    print_group("NVR", categories["nvr"])
    print_group("SWITCH", categories["switch"])
    print_group("BROADCAST", categories["broadcast"])
    print_group("DEVICE (lain)", categories["device"])
    print_group("UNKNOWN", categories["unknown"])

    print(f"\n{'='*60}")
    print("Selesai. Gunakan informasi di atas untuk identifikasi.")
    print(f"{'='*60}\n")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        cidr = sys.argv[1]
    else:
        cidr = get_local_cidr()
        if not cidr:
            print("[!] Gagal detect network. Berikan CIDR manual, misal:")
            print("    python network_scanner.py 192.168.1.0/24")
            sys.exit(1)
        print(f"[*] Auto-detect network: {cidr}")

    scan_network(cidr)
