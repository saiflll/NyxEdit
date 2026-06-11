package main

import (
	"fmt"
	"net"
	"strings"
	"time"
)

func main() {
	for _, ip := range []string{"10.201.30.2", "10.201.30.3", "10.201.30.4", "10.201.30.5"} {
		fmt.Printf("--- %s ---\n", ip)
		for _, p := range []int{80, 443, 554, 8000, 8443, 8080} {
			conn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%d", ip, p), 1*time.Second)
			if err != nil {
				continue
			}
			fmt.Printf("  Port %d OPEN", p)
			if p == 80 || p == 443 || p == 8000 || p == 8080 || p == 8443 {
				conn.SetDeadline(time.Now().Add(2 * time.Second))
				conn.Write([]byte(fmt.Sprintf("GET / HTTP/1.0\r\nHost: %s\r\n\r\n", ip)))
				buf := make([]byte, 1024)
				n, _ := conn.Read(buf)
				resp := string(buf[:n])
				for _, line := range strings.Split(resp, "\r\n") {
					low := strings.ToLower(line)
					if strings.HasPrefix(low, "server:") || strings.HasPrefix(low, "www-authenticate:") || strings.HasPrefix(low, "content-type:") {
						fmt.Printf(" | %s", line)
					}
				}
				// Check for title
				if strings.Contains(resp, "<title>") {
					parts := strings.Split(resp, "<title>")
					if len(parts) > 1 {
						title := strings.Split(parts[1], "</title>")[0]
						fmt.Printf(" | Title: %s", title)
					}
				}
				// Check first 200 chars of body for keywords
				bodyStart := strings.Index(resp, "\r\n\r\n")
				if bodyStart > 0 && bodyStart+4 < len(resp) {
					body := resp[bodyStart+4:]
					if len(body) > 200 { body = body[:200] }
					for _, kw := range []string{"NVR", "DVR", "Camera", "Recorder", "Hikvision", "Dahua", "iVMS", "Web Client"} {
						if strings.Contains(body, kw) {
							fmt.Printf(" | [BODY: %s]", kw)
							break
						}
					}
					fmt.Printf(" | [BODY] %s", strings.ReplaceAll(strings.ReplaceAll(body, "\n", " "), "\r", ""))
				}
			}
			fmt.Println()
			conn.Close()
		}
		fmt.Println()
	}
}
