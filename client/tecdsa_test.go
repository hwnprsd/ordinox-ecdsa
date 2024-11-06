package thresholdecdsa

import (
	"crypto/ecdsa"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"log"
	"net/url"
	"testing"

	"github.com/aviate-labs/agent-go"
	cmotoko "github.com/aviate-labs/agent-go/common/crust"
	"github.com/aviate-labs/agent-go/identity"
	"github.com/aviate-labs/agent-go/principal"
	"github.com/btcsuite/btcd/btcec/v2"
	becdsa "github.com/btcsuite/btcd/btcec/v2/ecdsa"
	"github.com/ethereum/go-ethereum/crypto"
)

const CANISTER_ID = "bkyz2-fmaaa-aaaaa-qaaaq-cai"

// const CANISTER_ID = "bd3sg-teaaa-aaaaa-qaaba-cai"

func mustDecodeHex(s string) []byte {
	b, err := hex.DecodeString(s)
	if err != nil {
		panic(err)
	}
	return b
}

var (
	// Hardcoded private keys (32 bytes each)
	privateKey1 = "85e1aa986dd173a50f9bd319caa3e7e7ab79d17423fca4c662902cdca23ba1b8"
	privateKey2 = "2e27369ef6bd0e45e06d27fc5081f2c7f9a2e71b21551e46a25185bca9a0477e"
	privateKey3 = "91f761ec5db294b486b05b1cd81e5eb5cf018178c84c8b7553951d126b796e02"
)

var u, _ = url.Parse("http://localhost:4943")
var config = agent.Config{
	ClientConfig:                   &agent.ClientConfig{Host: u},
	FetchRootKey:                   true,
	DisableSignedQueryVerification: true,
}

func createIdentityFromPrivateKey(privateKey ed25519.PrivateKey) identity.Identity {
	x, _ := identity.NewEd25519Identity(privateKey.Public().(ed25519.PublicKey), privateKey)
	return x
}

func TestECDSA(t *testing.T) {
	canisterID, _ := principal.Decode(CANISTER_ID)
	o1 := NewOrdinoxCanister(CANISTER_ID, privateKey1)
	o2 := NewOrdinoxCanister(CANISTER_ID, privateKey2)
	o3 := NewOrdinoxCanister(CANISTER_ID, privateKey3)

	a, err := agent.New(config)
	if err != nil {
		t.Fatalf("Failed to create agent: %v", err)
	}

	err = setupCanister(a, canisterID, []principal.Principal{o1.sender, o2.sender, o3.sender}, 2)
	if err != nil {
		t.Fatalf("failed to setup canister")
	}

	msg := "This is the end"

	pk, err := o1.GetPublicKey()
	fmt.Println("Pubkey", pk)
	address, err := o1.GetEvmAddress()
	fmt.Println("Address - ", address)
	return
	if err != nil {
		fmt.Println(err)
		t.Fatalf("error signing message, %e", err)
	}

	msgHash, err := o1.CreateOrSignMessage(msg)
	if err != nil {
		fmt.Println(err)
		t.Fatalf("error signing message, %e", err)
	}

	_, err = o2.CreateOrSignMessage(msg)
	if err != nil {
		fmt.Println(err)
	}

	_, err = o3.CreateOrSignMessage(msg)
	if err != nil {
		t.Fatalf("error signing message, %e", err)
	}

	sig, err := o1.GetSignature(msgHash)
	if err != nil {
		t.Fatalf("error signing message, %e", err)
	}

	fmt.Println(verifyEthereumSignatureHex(msg, pk, sig))
	// err = verifySignature(msg, pk, sig)
	// if err != nil {
	// 	t.Fatalf("error verifying signature, %e", err)
	// }

}

// decompressPubKey takes a compressed public key in hex and returns the uncompressed public key.
func decompressPubKey(pubKeyHex string) (*ecdsa.PublicKey, error) {
	// Decode the compressed public key
	pubKeyBytes, err := hex.DecodeString(pubKeyHex)
	if err != nil {
		return nil, fmt.Errorf("failed to decode public key hex: %v", err)
	}

	// Decompress the public key using btcec
	pubKey, err := btcec.ParsePubKey(pubKeyBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to decompress public key: %v", err)
	}

	// Convert to ecdsa.PublicKey format expected by go-ethereum
	return pubKey.ToECDSA(), nil
}

func verifyEthereumSignatureHex(message, pubKeyHex, signatureHex string) bool {
	// Decompress the public key
	pubKey, err := decompressPubKey(pubKeyHex)
	if err != nil {
		log.Printf("Error decompressing public key: %v", err)
		return false
	}

	// Decode the signature from hex
	signature, err := hex.DecodeString(signatureHex)
	if err != nil {
		log.Printf("Failed to decode signature hex: %v", err)
		return false
	}

	// Check that the signature is either 65 bytes (r, s, v) or 64 bytes (r, s)
	if len(signature) == 64 {
		// Try with `v` value as 27
		if verifyWithRecoveryID(pubKey, message, append(signature, 27)) {
			return true
		}
		// Try with `v` value as 28
		return verifyWithRecoveryID(pubKey, message, append(signature[:64], 28))
	} else if len(signature) == 65 {
		// If `v` is already included, attempt verification directly
		return verifyWithRecoveryID(pubKey, message, signature)
	} else {
		log.Println("Invalid signature format: expected 65 bytes (r, s, v) or 64 bytes")
		return false
	}
}

// verifyWithRecoveryID performs verification with a specific `v` value included in the signature.
func verifyWithRecoveryID(pubKey *ecdsa.PublicKey, message string, signature []byte) bool {
	// Hash the message using Keccak-256
	hash := crypto.Keccak256([]byte(message))

	// Attempt to recover the public key from the signature
	recoveredPubKey, err := crypto.SigToPub(hash, signature)
	if err != nil {
		log.Printf("Error recovering public key from signature: %v", err)
		return false
	}

	// Compare the recovered public key to the provided public key
	return recoveredPubKey.Equal(pubKey)
}

func verifySignature(message, pubkeyHex, sigHex string) error {
	pubKeyBytes, err := hex.DecodeString(pubkeyHex)
	if err != nil {
		fmt.Println("error decoding pubkey hex", pubkeyHex)
		return err
	}

	hash := sha256.Sum256([]byte(message))

	// Deserialize the public key
	pubKey, err := btcec.ParsePubKey(pubKeyBytes)
	if err != nil {
		fmt.Println("Failed to parse public key:", err)
		return err
	}

	signatureBytes, err := hex.DecodeString(sigHex)
	if err != nil {
		fmt.Println("error decoding sig hex", sigHex)
		return err
	}

	// Extract R and S from the signature bytes (assuming raw R|S format)
	if len(signatureBytes) != 64 {
		fmt.Println("Invalid signature length, expected 64 bytes")
		return nil
	}
	// Convert R and S to modNScalar
	var r, s btcec.ModNScalar
	if overflow := r.SetByteSlice(signatureBytes[:32]); overflow {
		fmt.Println("R value overflow")
		return fmt.Errorf("r overflow")
	}
	if overflow := s.SetByteSlice(signatureBytes[32:]); overflow {
		fmt.Println("S value overflow")
		return fmt.Errorf("s overflow")
	}
	// Create the ECDSA signature using R and S
	signature := becdsa.NewSignature(&r, &s)

	// Verify the signature
	isValid := signature.Verify(hash[:], pubKey)
	fmt.Println("Signature valid:", isValid)
	return nil
}

func _TestThresholdECDSACanister(t *testing.T) {
	// Create identities from seed phrases
	// id1 := createIdentityFromPrivateKey(privateKey1)
	// id2 := createIdentityFromPrivateKey(privateKey2)
	// id3 := createIdentityFromPrivateKey(privateKey3)
	id1, err := identity.NewRandomEd25519Identity()
	if err != nil {
		panic(err)
	}
	id2, err := identity.NewRandomEd25519Identity()
	if err != nil {
		panic(err)
	}
	id3, err := identity.NewRandomEd25519Identity()
	if err != nil {
		panic(err)
	}

	// Get principals for the identities
	p1 := id1.Sender()
	p2 := id2.Sender()
	p3 := id3.Sender()

	// Connect to the local replica
	a, err := agent.New(config)
	if err != nil {
		t.Fatalf("Failed to create agent: %v", err)
	}

	// Create a new canister
	// canisterID, err := createCanister(a)
	// if err != nil {
	// 	t.Fatalf("Failed to create canister: %v", err)
	// }
	canisterID, _ := principal.Decode(CANISTER_ID)

	fmt.Printf("Created canister with ID: %s\n", canisterID)

	// Setup the canister
	err = setupCanister(a, canisterID, []principal.Principal{p1, p2, p3}, 2)
	if err != nil {
		t.Fatalf("Failed to setup canister: %v", err)
	}

	msg := "0xhelloworld"
	// Create a message with the first identity
	hash, err := createOrSignMessage(id1, canisterID, msg)
	if err != nil {
		t.Fatalf("Failed to create message: %v", err)
	}
	fmt.Printf("Created message: %s\n%s", msg, hash)

	// Sign the message with the second identity
	_, err = createOrSignMessage(id2, canisterID, msg)
	if err != nil {
		t.Fatalf("Failed to sign message: %v", err)
	}
	fmt.Println("Signed message with second identity")

	// Get the message
	signature, err := getSignature(id1, canisterID, hash)
	if err != nil {
		t.Fatalf("Failed to get message: %v", err)
	}
	fmt.Printf("Retrieved message: %+v\n", signature)
}

func createCanister(a *agent.Agent) (principal.Principal, error) {
	var result struct{ CanisterID principal.Principal }
	managementCanister := principal.Principal{Raw: []byte{0x0}}
	err := a.Call(managementCanister, "create_canister", []any{createCanisterArgs{}}, []any{&result})
	if err != nil {
		return principal.Principal{}, err
	}

	if err != nil {
		return principal.Principal{}, err
	}

	return result.CanisterID, nil
}

func setupCanister(a *agent.Agent, canisterID principal.Principal, signers []principal.Principal, threshold uint32) error {
	res := ""
	err := a.Call(canisterID, "setup", []any{signers, threshold}, []any{&res})
	fmt.Println("Canister setup complete")
	return err
}

func createOrSignMessage(id identity.Identity, canisterID principal.Principal, message string) (string, error) {
	cfg := config
	cfg.Identity = id
	a, _ := agent.New(cfg)
	fmt.Println(cfg)
	var msg cmotoko.Result[string, string]
	// create new agent and then call
	err := a.Call(canisterID, "create_or_sign_message", []any{message}, []any{&msg})
	if err != nil {
		return "x", err
	}
	return *msg.Ok, err
}

func getSignature(id identity.Identity, canisterID principal.Principal, msg string) (string, error) {
	cfg := config
	cfg.Identity = id
	a, _ := agent.New(cfg)

	var result string
	err := a.Query(canisterID, "get_signature", []any{msg}, []any{&result})
	if err != nil {
		return "", fmt.Errorf("query failed: %v", err)
	}
	return result, nil
}

type Message struct {
	ID        uint64
	Data      string
	Signers   []principal.Principal
	Signature string
}

type createCanisterArgs struct{}
