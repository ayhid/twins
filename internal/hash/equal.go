package hash

import (
	"bytes"
	"fmt"
	"io"
	"os"
)

// Equal compares two files byte by byte. It is the final, hash-free
// verification used before permanent deletion.
func Equal(a, b string) (bool, error) {
	fa, err := os.Open(a)
	if err != nil {
		return false, fmt.Errorf("open: %w", err)
	}
	defer fa.Close()
	fb, err := os.Open(b)
	if err != nil {
		return false, fmt.Errorf("open: %w", err)
	}
	defer fb.Close()

	bufA := buffers.Get().(*[]byte)
	defer buffers.Put(bufA)
	bufB := buffers.Get().(*[]byte)
	defer buffers.Put(bufB)

	for {
		na, errA := io.ReadFull(fa, *bufA)
		nb, errB := io.ReadFull(fb, *bufB)
		if na != nb || !bytes.Equal((*bufA)[:na], (*bufB)[:nb]) {
			return false, nil
		}
		if errA == io.EOF || errA == io.ErrUnexpectedEOF {
			return errB == io.EOF || errB == io.ErrUnexpectedEOF, nil
		}
		if errA != nil {
			return false, fmt.Errorf("read %s: %w", a, errA)
		}
		if errB != nil {
			return false, fmt.Errorf("read %s: %w", b, errB)
		}
	}
}
