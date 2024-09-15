package main

import (
	"crypto/ed25519"
	"encoding/hex"
	"fmt"
	"net/url"
	"testing"

	"github.com/aviate-labs/agent-go"
	cmotoko "github.com/aviate-labs/agent-go/common/crust"
	"github.com/aviate-labs/agent-go/identity"
	"github.com/aviate-labs/agent-go/principal"
)

func mustDecodeHex(s string) []byte {
	b, err := hex.DecodeString(s)
	if err != nil {
		panic(err)
	}
	return b
}

var (
	// Hardcoded private keys (32 bytes each)
	privateKey1 = ed25519.PrivateKey(mustDecodeHex("6006a870666ac4150f35434c15ae8c7122e6ec45d7717c632adc9843338f721136827b6f85f401a0d433fdfbb939835572c4047b53b3c19acc616e6ca6be981e"))
	privateKey2 = ed25519.PrivateKey(mustDecodeHex("3d48910f03f80c6ee1042cc416f775abce78e3655b81374ca3eb8a1205fa795aa4a13332cbf4f97d7313ddadc5457841add0bcc66caeb23339fd953df7b69fb1"))
	privateKey3 = ed25519.PrivateKey(mustDecodeHex("a599d7f971416499a09cc3be58118c0e9a15b6becb9d1f7de89de21920db6ace9e30fb9de47d124c2e0b349aae138a50b500bc3ef5c9cbb6f6d6b4869437d94c"))
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

func TestThresholdECDSACanister(t *testing.T) {
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
	canisterID, _ := principal.Decode("br5f7-7uaaa-aaaaa-qaaca-cai")

	fmt.Printf("Created canister with ID: %s\n", canisterID)

	// Setup the canister
	err = setupCanister(a, canisterID, []principal.Principal{p1, p2, p3}, 2)
	if err != nil {
		t.Fatalf("Failed to setup canister: %v", err)
	}
	fmt.Println("Canister setup complete")

	msg := "0xhelloworld"
	// Create a message with the first identity
	_, err = createOrSignMessage(id1, canisterID, msg)
	if err != nil {
		t.Fatalf("Failed to create message: %v", err)
	}
	fmt.Printf("Created message: %s\n", msg)

	// Sign the message with the second identity
	_, err = createOrSignMessage(id2, canisterID, msg)
	if err != nil {
		t.Fatalf("Failed to sign message: %v", err)
	}
	fmt.Println("Signed message with second identity")

	// Get the message
	signature, err := getSignature(id1, canisterID, msg)
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
	return err
}

func createOrSignMessage(id identity.Identity, canisterID principal.Principal, message string) (uint64, error) {
	cfg := config
	cfg.Identity = id
	a, _ := agent.New(cfg)
	var msg cmotoko.Result[uint64, string]
	// create new agent and then call
	err := a.Call(canisterID, "create_or_sign_message", []any{message}, []any{&msg})
	if err != nil {
		return 0, err
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
