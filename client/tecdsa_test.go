package ordinox_ecdsa_canister

import (
	"crypto/ed25519"
	"encoding/hex"
	"fmt"
	"github.com/aviate-labs/agent-go"
	"github.com/aviate-labs/agent-go/identity"
	"github.com/aviate-labs/agent-go/principal"
	"github.com/stretchr/testify/assert"
	"net/url"
	"testing"
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
	o1 := NewOrdinoxCanister(CANISTER_ID, privateKey1, RPC_LOCAL)
	o2 := NewOrdinoxCanister(CANISTER_ID, privateKey2, RPC_LOCAL)
	o3 := NewOrdinoxCanister(CANISTER_ID, privateKey3, RPC_LOCAL)

	a, err := agent.New(config)
	if err != nil {
		t.Fatalf("Failed to create agent: %v", err)
	}

	err = setupCanister(a, canisterID, []principal.Principal{o1.sender, o2.sender, o3.sender}, 2)
	if err != nil {
		t.Fatalf("failed to setup canister: %v", err)
	}

	address, err := o1.GetEvmAddress()

	msg := NewEvmTransferMessage(
		1, 1, "0x000000000000000000000000000000000000beef", "0x000000000000000000000000000000000000dead", "1",
	)

	msgHash, err := o1.CreateOrSignEvmMessage(msg)
	if err != nil {
		fmt.Println(err)
		t.Fatalf("error signing message, %e", err)
	}

	_, err = o2.CreateOrSignEvmMessage(msg)
	if err != nil {
		t.Fatalf("error signing message, %e", err)
	}

	_, err = o3.CreateOrSignEvmMessage(msg)
	if err != nil {
		t.Fatalf("error signing message, %e", err)
	}

	sig, err := o1.GetSignature(msgHash)
	if err != nil {
		t.Fatalf("error signing message, %e", err)
	}

	fmt.Println("Address:", address)
	fmt.Println("MessageHex:", msgHash)
	// sigHex := constructSignature(sig)

	fmt.Println("SigHex:", sig)
	verified, err := verifyEvmSig(address, msgHash, sig)
	if err != nil {
		t.Fatalf("error verifying signature, %e", err)
	}
	assert.True(t, verified, "signature verification should pass")
}

func setupCanister(a *agent.Agent, canisterID principal.Principal, signers []principal.Principal, threshold uint32) error {
	res := ""
	err := a.Call(canisterID, "setup", []any{signers, threshold}, []any{&res})

	return err

}
