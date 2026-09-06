package fsutil

import (
	"fmt"
	"strconv"
	"strings"
)

var sizeUnits = []string{"B", "KiB", "MiB", "GiB", "TiB", "PiB"}

// HumanSize renders a byte count using binary units (1 KiB = 1024 B).
func HumanSize(n int64) string {
	if n < 1024 {
		return fmt.Sprintf("%d B", n)
	}
	value := float64(n)
	unit := 0
	for value >= 1024 && unit < len(sizeUnits)-1 {
		value /= 1024
		unit++
	}
	return fmt.Sprintf("%.1f %s", value, sizeUnits[unit])
}

var sizeMultipliers = map[string]int64{
	"":    1,
	"b":   1,
	"k":   1 << 10,
	"kb":  1 << 10,
	"kib": 1 << 10,
	"m":   1 << 20,
	"mb":  1 << 20,
	"mib": 1 << 20,
	"g":   1 << 30,
	"gb":  1 << 30,
	"gib": 1 << 30,
	"t":   1 << 40,
	"tb":  1 << 40,
	"tib": 1 << 40,
}

// ParseSize parses a human size such as "512", "10K", "1.5MiB" or "2 GB".
// Units are binary (1K = 1024) regardless of spelling.
func ParseSize(s string) (int64, error) {
	s = strings.TrimSpace(strings.ToLower(s))
	i := strings.IndexFunc(s, func(r rune) bool { return (r < '0' || r > '9') && r != '.' })
	num, unit := s, ""
	if i >= 0 {
		num, unit = s[:i], strings.TrimSpace(s[i:])
	}
	mult, ok := sizeMultipliers[unit]
	if !ok || num == "" {
		return 0, fmt.Errorf("invalid size %q", s)
	}
	value, err := strconv.ParseFloat(num, 64)
	if err != nil || value < 0 {
		return 0, fmt.Errorf("invalid size %q", s)
	}
	return int64(value * float64(mult)), nil
}
