pragma solidity 0.8.27;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol";


// ERC20 Interface
interface iERC20 {
    function balanceOf(address) external view returns (uint256);
    function burn(uint) external;
}
// ROUTER Interface
interface iROUTER {
    function depositWithExpiry(address, address, uint, string calldata, uint) external;
}

// RDNX_Router is managed by RDNX Vaults
contract RdnxRouter {
    using ECDSA for bytes32;
    using MessageHashUtils for bytes32;

    struct Coin {
        address asset;
        uint amount;
    }

    // Vault allowance for each asset
    mapping(address => mapping(address => uint)) private _vaultAllowance;

    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED = 2;
    uint256 private _status;

    // Emitted for all deposits, the memo distinguishes for swap, add, remove, donate etc
    event Deposit(address indexed to, address indexed asset, uint amount, string memo);

    // Emitted for all outgoing transfers, the vault dictates who sent it, memo used to track.
    event TransferOut(address indexed vault, address indexed to, address asset, uint amount, string memo);

    // Emitted for all outgoing transferAndCalls, the vault dictates who sent it, memo used to track.
    event TransferOutAndCall(address indexed vault, address target, uint amount, address finalAsset, address to, uint256 amountOutMin, string memo);

    // Changes the spend allowance between vaults
    event TransferAllowance(address indexed oldVault, address indexed newVault, address asset, uint amount, string memo);

    // Specifically used to batch send the entire vault assets
    event VaultTransfer(address indexed oldVault, address indexed newVault, Coin[] coins, string memo);

    modifier nonReentrant() {
        require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }

    constructor() {
        _status = _NOT_ENTERED;
    }

    // Deposit with Expiry (preferred)
    function depositWithExpiry(address vault, address asset, uint amount, string memory memo, uint expiration) external {
        require(block.timestamp < expiration, "RDNX_Router: expired");
        deposit(vault, asset, amount, memo);
    }

    // Deposit an asset with a memo. ETH is forwarded, ERC-20 stays in ROUTER
    function deposit(address vault, address asset, uint amount, string memory memo) public nonReentrant{
        uint safeAmount;
                safeAmount = safeTransferFrom(asset, amount); // Transfer asset
                _vaultAllowance[vault][asset] += safeAmount; // Credit to chosen vault
        emit Deposit(vault, asset, safeAmount, memo);
    }

    //############################## ALLOWANCE TRANSFERS ##############################

    // Use for "moving" assets between vaults (rdnxVault<>ygg), as well "churning" to a new rdnxVault
    function transferAllowance(address router, address newVault, address asset, uint amount, string memory memo) external nonReentrant {
        if (router == address(this)){
            _adjustAllowances(newVault, asset, amount);
            emit TransferAllowance(msg.sender, newVault, asset, amount, memo);
        } else {
            _routerDeposit(router, newVault, asset, amount, memo);
        }
    }

    //############################## ASSET TRANSFERS ##############################

    // Any vault calls to transfer any asset to any recipient.
    // Note: Contract recipients of ETH are only given 2300 Gas to complete execution.
    function transferOut(address to, address asset, uint amount, string memory memo) public nonReentrant {
        uint safeAmount;
            _vaultAllowance[msg.sender][asset] -= amount; // Reduce allowance
            (bool success, bytes memory data) = asset.call(abi.encodeWithSignature("transfer(address,uint256)" , to, amount));
            require(success && (data.length == 0 || abi.decode(data, (bool))));
            safeAmount = amount;
        emit TransferOut(msg.sender, to, asset, safeAmount, memo);
    }

    function transferOutWithSignature(
        address from, 
        address to, 
        address token, 
        uint amount, 
        uint128 chainId, 
        uint128 nonce, 
        bytes calldata signature
    ) public view returns (address recoveredAddress) {
        bytes32 message = keccak256(
            abi.encodePacked(
                nonce,
                chainId,
                token,
                to,
                amount
            )
        );

        bytes32 messageHash = MessageHashUtils.toEthSignedMessageHash(message);
        recoveredAddress = messageHash.recover(signature);
    }

    //############################## VAULT MANAGEMENT ##############################

    // A vault can call to "return" all assets to an rdnxVault, including ETH. 
    function returnVaultAssets(address router, address rdnxVault, Coin[] memory coins, string memory memo) external nonReentrant {
        if (router == address(this)){
            for(uint i = 0; i < coins.length; i++){
                _adjustAllowances(rdnxVault, coins[i].asset, coins[i].amount);
            }
            emit VaultTransfer(msg.sender, rdnxVault, coins, memo); // Does not include ETH.           
        } else {
            for(uint i = 0; i < coins.length; i++){
                _routerDeposit(router, rdnxVault, coins[i].asset, coins[i].amount, memo);
            }
        }
    }

    //############################## HELPERS ##############################

    function vaultAllowance(address vault, address token) public view returns(uint amount){
        return _vaultAllowance[vault][token];
    }

    // Safe transferFrom in case asset charges transfer fees
    function safeTransferFrom(address _asset, uint _amount) internal returns(uint amount) {
        uint _startBal = iERC20(_asset).balanceOf(address(this));
        (bool success, bytes memory data) = _asset.call(abi.encodeWithSignature("transferFrom(address,address,uint256)", msg.sender, address(this), _amount));
        require(success && (data.length == 0 || abi.decode(data, (bool))));
        return (iERC20(_asset).balanceOf(address(this)) - _startBal);
    }

    // Decrements and Increments Allowances between two vaults
    function _adjustAllowances(address _newVault, address _asset, uint _amount) internal {
        _vaultAllowance[msg.sender][_asset] -= _amount;
        _vaultAllowance[_newVault][_asset] += _amount;
    }

    // Adjust allowance and forwards funds to new router, credits allowance to desired vault
    function _routerDeposit(address _router, address _vault, address _asset, uint _amount, string memory _memo) internal {
        _vaultAllowance[msg.sender][_asset] -= _amount;
        (bool success,) = _asset.call(abi.encodeWithSignature("approve(address,uint256)", _router, _amount)); // Approve to transfer
        require(success);
        iROUTER(_router).depositWithExpiry(_vault, _asset, _amount, _memo, type(uint).max); // Transfer by depositing
    }
}

