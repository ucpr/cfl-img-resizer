package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
)

type Query struct {
	FullPath string
}

func (q *Query) Token(secret string) string {
	hasher := sha256.New()
	hasher.Write([]byte(q.FullPath))
	hasher.Write([]byte("$"))
	hasher.Write([]byte(secret))

	hash := hasher.Sum(nil)
	encodedHash := hex.EncodeToString(hash)

	return encodedHash
}

func main() {
	// pathFormat := "/?width=%d&height=%d"
	ys := Query{
		FullPath: "/?width=100&height=200&blur=3",
	}

	token := ys.Token(os.Getenv("TOKEN_SECRET"))
	fmt.Println(token)
}
