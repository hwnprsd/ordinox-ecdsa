package ordinox_ecdsa_canister

import (
	"fmt"
	"log"
	"math/big"
	"regexp"
	"strings"

	"github.com/ethereum/go-ethereum/crypto"
)

func parseInput(input string) (byte, *big.Int, *big.Int, error) {
	re := regexp.MustCompile(`v:\s*Parity\((true|false)\),\s*r:\s*(\d+),\s*s:\s*(\d+)`)
	matches := re.FindStringSubmatch(input)

	if len(matches) < 4 {
		return 0, nil, nil, fmt.Errorf("input format incorrect")
	}

	// Parse v
	v := byte(27)
	if matches[1] == "true" {
		v = 28
	}

	// Parse r and s
	r, success := new(big.Int).SetString(matches[2], 10)
	if !success {
		return 0, nil, nil, fmt.Errorf("invalid r value")
	}
	s, success := new(big.Int).SetString(matches[3], 10)
	if !success {
		return 0, nil, nil, fmt.Errorf("invalid s value")
	}

	return v, r, s, nil
}

// Function to verify the EVM signature
func verifyEvmSig(address, message string, sig string) bool {
	v, r, s, err := parseInput(sig)
	if err != nil {
		log.Fatalf("err parsing input: %v", err)
	}

	// Adjust v to be either 0 or 1 as expected by SigToPub
	if v >= 27 {
		v -= 27
	}

	prefixedMsg := fmt.Sprintf("\x19Ethereum Signed Message:\n%d%s", len(message), message)
	hash := crypto.Keccak256Hash([]byte(prefixedMsg))

	// Create the 65-byte signature from r, s, v (adding 27 to align with Ethereum convention)
	signature := make([]byte, 65)
	copy(signature[0:32], r.Bytes())
	copy(signature[32:64], s.Bytes())
	signature[64] = v

	// Recover public key from signature
	publicKey, err := crypto.SigToPub(hash.Bytes(), signature)
	if err != nil {
		log.Fatalf("Failed to recover public key: %v", err)
	}

	recoveredAddress := crypto.PubkeyToAddress(*publicKey)
	return strings.EqualFold(recoveredAddress.Hex(), address)
}
