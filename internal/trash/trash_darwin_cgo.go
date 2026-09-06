//go:build darwin && cgo

package trash

/*
#cgo CFLAGS: -x objective-c -fobjc-arc
#cgo LDFLAGS: -framework Foundation

#import <Foundation/Foundation.h>

// trashItem moves path to the Trash. Returns 0 on success, otherwise the
// NSError code and fills errbuf with the description.
static int trashItem(const char *path, char *errbuf, int errlen) {
	@autoreleasepool {
		NSString *p = [NSString stringWithUTF8String:path];
		NSURL *url = [NSURL fileURLWithPath:p];
		NSError *error = nil;
		BOOL ok = [[NSFileManager defaultManager] trashItemAtURL:url resultingItemURL:nil error:&error];
		if (ok) {
			return 0;
		}
		if (error != nil) {
			strncpy(errbuf, [[error localizedDescription] UTF8String], errlen - 1);
			errbuf[errlen - 1] = 0;
			return (int)[error code] ?: -1;
		}
		return -1;
	}
}
*/
import "C"

import (
	"errors"
	"unsafe"
)

const trashBackend = "NSFileManager"

func moveToTrash(path string) error {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))
	buf := make([]byte, 512)
	code := C.trashItem(cpath, (*C.char)(unsafe.Pointer(&buf[0])), C.int(len(buf)))
	if code == 0 {
		return nil
	}
	msg := C.GoString((*C.char)(unsafe.Pointer(&buf[0])))
	if msg == "" {
		msg = "trashItemAtURL failed"
	}
	return errors.New("trash: " + msg)
}
