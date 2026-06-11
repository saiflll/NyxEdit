package main

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"sort"
	"strings"
	"sync"
	"time"
)

type DeviceInfo struct {
	IP     string
	MAC    string
	Vendor string
	Type   string
	Ports  []int
	Banner string
}

func main() {
	t0 := time.Now()

	subnet := "10.201.30"
	if len(os.Args) > 1 {
		subnet = os.Args[1]
	}
	fmt.Printf("Scan %s.1-254 ...\n\n", subnet)

	arp := getARP()

	var mu sync.Mutex
	var devices []DeviceInfo
	sem := make(chan struct{}, 500)
	var wg sync.WaitGroup

	for i := 1; i <= 254; i++ {
		wg.Add(1)
		sem <- struct{}{}
		go func(ip string) {
			defer wg.Done()
			defer func() { <-sem }()
			if !isUp(ip) {
				return
			}
			mac := arp[ip]
			ports := scanPorts(ip)
			banner := grabBanner(ip, ports)
			vendor, dtype := match(mac, ports, banner)
			mu.Lock()
			devices = append(devices, DeviceInfo{ip, mac, vendor, dtype, ports, banner})
			mu.Unlock()
			fmt.Printf("  %-15s", ip)
			if mac != "" {
				fmt.Printf(" %s", mac)
			}
			if vendor != "" {
				fmt.Printf(" [%s]", vendor)
			}
			if len(ports) > 0 {
				fmt.Printf(" %v", ports)
			}
			fmt.Println()
		}(fmt.Sprintf("%s.%d", subnet, i))
	}
	wg.Wait()

	sort.Slice(devices, func(i, j int) bool { return devices[i].IP < devices[j].IP })

	groups := map[string][]DeviceInfo{}
	for _, d := range devices {
		groups[d.Type] = append(groups[d.Type], d)
	}

	fmt.Printf("\n=== HASIL (%d ditemukan, %v) ===\n", len(devices), time.Since(t0).Round(time.Millisecond))
	for _, cat := range []string{"camera", "nvr", "switch", "broadcast"} {
		list := groups[cat]
		if len(list) == 0 {
			continue
		}
		fmt.Printf("\n■■ %s (%d)\n", strings.ToUpper(cat), len(list))
		for _, d := range list {
			fmt.Printf("  %-15s %s  [%s]\n", d.IP, padMAC(d.MAC), d.Vendor)
		}
		delete(groups, cat)
	}
	for cat, list := range groups {
		fmt.Printf("\n■■ %s (%d)\n", strings.ToUpper(cat), len(list))
		for _, d := range list {
			fmt.Printf("  %-15s %s  [%s]\n", d.IP, padMAC(d.MAC), d.Vendor)
		}
	}
	fmt.Print("\nPress Enter to exit...")
	fmt.Scanln()
}

func isUp(ip string) bool {
	ch := make(chan struct{}, 3)
	var ok bool
	var mu sync.Mutex
	var wg sync.WaitGroup
	for _, p := range []int{80, 443, 554, 22, 8080, 8000} {
		wg.Add(1)
		ch <- struct{}{}
		go func(port int) {
			defer wg.Done()
			defer func() { <-ch }()
			conn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%d", ip, port), 300*time.Millisecond)
			if err != nil {
				return
			}
			conn.Close()
			mu.Lock()
			ok = true
			mu.Unlock()
		}(p)
	}
	wg.Wait()
	return ok
}

var portsToCheck = []int{80, 443, 554, 22, 23, 8080, 8000, 8443, 7000, 37777, 34567, 8554, 161, 5000, 21, 445, 3389, 5900, 8448, 1900}

func scanPorts(ip string) []int {
	var mu sync.Mutex
	var ports []int
	var wg sync.WaitGroup
	sem := make(chan struct{}, 50)
	for _, p := range portsToCheck {
		wg.Add(1)
		sem <- struct{}{}
		go func(port int) {
			defer wg.Done()
			defer func() { <-sem }()
			conn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%d", ip, port), 500*time.Millisecond)
			if err != nil {
				return
			}
			conn.Close()
			mu.Lock()
			ports = append(ports, port)
			mu.Unlock()
		}(p)
	}
	wg.Wait()
	sort.Ints(ports)
	return ports
}

func grabBanner(ip string, ports []int) string {
	for _, p := range ports {
		if p != 80 && p != 443 && p != 8000 && p != 8080 && p != 8443 {
			continue
		}
		conn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%d", ip, p), 1*time.Second)
		if err != nil {
			continue
		}
		conn.SetDeadline(time.Now().Add(1 * time.Second))
		conn.Write([]byte(fmt.Sprintf("GET / HTTP/1.0\r\nHost: %s\r\n\r\n", ip)))
		buf := make([]byte, 512)
		n, _ := conn.Read(buf)
		conn.Close()
		if n == 0 {
			continue
		}
		resp := string(buf[:n])
		for _, line := range strings.Split(resp, "\r\n") {
			if len(line) > 7 && strings.HasPrefix(strings.ToLower(line), "server:") {
				return strings.TrimSpace(line[7:])
			}
		}
	}
	return ""
}

func match(mac string, ports []int, banner string) (vendor string, dtype string) {
	b := strings.ToLower(banner)
	if strings.Contains(b, "hikvision") || strings.Contains(b, "dahua") {
		return banner, "camera"
	}
	if strings.Contains(b, "axis") {
		return "Axis", "camera"
	}
	if strings.Contains(b, "switch") || strings.Contains(b, "smart") {
		return "", "switch"
	}
	if strings.Contains(b, "broadcast") || strings.Contains(b, "public address") {
		return "", "broadcast"
	}

	m := strings.ReplaceAll(strings.ToLower(mac), ":", "")
	m = strings.ReplaceAll(m, "-", "")

	pref := []struct{ p, v, t string }{
		{"001b1c", "Hikvision", "camera"}, {"0c37dc", "Hikvision", "camera"},
		{"4419b6", "Hikvision", "camera"}, {"8cea1b", "Hikvision", "camera"},
		{"3ce36b", "Hikvision", "camera"}, {"bcad28", "Hikvision", "camera"},
		{"000fe2", "Dahua", "camera"}, {"a84a05", "Dahua", "camera"},
		{"247e12", "Uniview", "camera"}, {"001a4f", "Axis", "camera"},
		{"00129b", "Mobotix", "camera"}, {"001892", "ACTi", "camera"},
		{"000fb5", "3Com", "switch"}, {"68d79a", "TP-Link", "switch"},
		{"a0f3e4", "TP-Link", "switch"}, {"00065b", "Netgear", "switch"},
		{"0014bf", "Cisco", "switch"}, {"4c5e0c", "MikroTik", "switch"},
		{"b827eb", "RPi", "pc"}, {"dca632", "RPi", "pc"},
		{"08005c", "Ubiquiti", "switch"}, {"000c29", "VMware", "virtual"},
		{"005056", "VMware", "virtual"}, {"d85de2", "Zyxel", "switch"},
	}
	for _, v := range pref {
		if strings.HasPrefix(m, v.p) {
			return v.v, v.t
		}
	}

	has := func(p int) bool {
		for _, x := range ports {
			if x == p {
				return true
			}
		}
		return false
	}
	// NVR: usually has RTSP + HTTP, often NO Dahua port 8000, NO HTTPS
	// vs Cameras: usually have Dahua port 8000 + often HTTPS
	if (has(554) || has(8554)) && !has(8000) && !has(443) {
		return "", "nvr"
	}
	// Camera: has RTSP + Dahua port 8000, often also HTTPS
	if has(8000) || has(37777) || has(34567) || has(7000) {
		return "", "camera"
	}
	if has(554) || has(8554) {
		return "", "camera"
	}
	if has(23) || has(161) {
		return "", "switch"
	}
	if has(80) || has(443) {
		return "", "device"
	}
	if has(3389) || has(445) {
		return "", "windows"
	}
	if has(22) {
		return "", "linux"
	}
	return "", "unknown"
}

func padMAC(mac string) string {
	if len(mac) < 17 {
		return mac + strings.Repeat(" ", 17-len(mac))
	}
	return mac
}

func getARP() map[string]string {
	out, err := exec.Command("arp", "-a").Output()
	if err != nil {
		return map[string]string{}
	}
	m := map[string]string{}
	for _, line := range strings.Split(string(out), "\n") {
		f := strings.Fields(line)
		if len(f) >= 3 && strings.Count(f[0], ".") == 3 && strings.Count(f[1], "-") == 5 {
			m[f[0]] = f[1]
		}
	}
	return m
}
