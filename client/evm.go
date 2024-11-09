package ordinox_ecdsa_canister

type EvmTransferMessage struct {
	ToAddress    string
	TokenAddress string
	Amount       string
	ChainId      uint64
	Nonce        uint64
}

func NewEvmTransferMessage(nonce, chainId uint64, toAddr, tokenAddr, amount string) EvmTransferMessage {
	return EvmTransferMessage{
		ToAddress:    toAddr,
		TokenAddress: tokenAddr,
		Amount:       amount,
		Nonce:        nonce,
		ChainId:      chainId,
	}
}

func (e EvmTransferMessage) Hex() string {
	return "Not Implemented"
}
