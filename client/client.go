package ordinox_ecdsa_canister

import (
	"crypto/ed25519"
	"encoding/hex"
	"fmt"
	"net/url"

	"github.com/aviate-labs/agent-go"
	cmotoko "github.com/aviate-labs/agent-go/common/crust"
	"github.com/aviate-labs/agent-go/identity"
	"github.com/aviate-labs/agent-go/principal"
)

type OrdinoxCanisterClient struct {
	canisterId principal.Principal
	sender     principal.Principal
	nodeId     identity.Identity
	cfg        agent.Config
}

func decodeHex(s string) []byte {
	b, err := hex.DecodeString(s)
	if err != nil {
		panic(err)
	}
	return b
}

func NewOrdinoxCanister(id string, pkHex string) OrdinoxCanisterClient {
	var icpUrl, _ = url.Parse("http://localhost:4943")
	canisterId, err := principal.Decode(id)
	if err != nil {
		panic(err)
	}

	privateKey := ed25519.NewKeyFromSeed(decodeHex(pkHex))
	nodeId, err := identity.NewEd25519Identity(privateKey.Public().(ed25519.PublicKey), privateKey)

	var config = agent.Config{
		ClientConfig:                   &agent.ClientConfig{Host: icpUrl},
		FetchRootKey:                   true,
		DisableSignedQueryVerification: true,
		Identity:                       nodeId,
	}

	if err != nil {
		panic(err)
	}

	return OrdinoxCanisterClient{
		canisterId: canisterId,
		nodeId:     nodeId,
		cfg:        config,
		sender:     nodeId.Sender(),
	}
}

func (o *OrdinoxCanisterClient) CreateOrSignEvmMessage(message EvmTransferMessage) (string, error) {
	a, _ := agent.New(o.cfg)
	var msg cmotoko.Result[string, string]
	err := a.Call(
		o.canisterId,
		"create_or_sign_evm_message",
		[]any{
			message.Nonce,
			message.ChainId,
			message.TokenAddress,
			message.ToAddress,
			message.Amount,
		},
		[]any{&msg},
	)
	if err != nil || msg.Ok == nil {
		return "x", err
	}
	return *msg.Ok, err
}

func (o *OrdinoxCanisterClient) GetSignature(msgId string) (string, error) {
	a, _ := agent.New(o.cfg)

	var result string
	err := a.Query(o.canisterId, "get_signature", []any{msgId}, []any{&result})
	if err != nil {
		return "", fmt.Errorf("query failed: %v", err)
	}
	return result, nil
}

func (o *OrdinoxCanisterClient) GetPublicKey() (string, error) {
	a, _ := agent.New(o.cfg)
	var msg cmotoko.Result[string, string]
	err := a.Call(o.canisterId, "public_key", []any{}, []any{&msg})
	if err != nil {
		return "x", err
	}
	return *msg.Ok, err
}

func (o *OrdinoxCanisterClient) GetEvmPublicKey() (string, error) {
	a, _ := agent.New(o.cfg)
	var msg cmotoko.Result[string, string]
	err := a.Call(o.canisterId, "evm_pub_key", []any{}, []any{&msg})
	if err != nil {
		return "x", err
	}
	return *msg.Ok, err
}

func (o *OrdinoxCanisterClient) GetEvmAddress() (string, error) {
	a, _ := agent.New(o.cfg)
	var msg cmotoko.Result[string, string]
	err := a.Call(o.canisterId, "evm_address", []any{}, []any{&msg})
	if err != nil {
		return "x", err
	}
	return *msg.Ok, err
}
