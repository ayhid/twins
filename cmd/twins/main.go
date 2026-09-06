// Command twins finds and removes duplicate files on macOS.
package main

import (
	"os"

	"github.com/ayhid/twins/internal/cli"
)

func main() {
	os.Exit(cli.Main(os.Args[1:]))
}
