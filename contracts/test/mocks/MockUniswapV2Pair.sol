// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract MockUniswapV2Pair {
    address public token0;
    address public token1;
    uint112 private _reserve0;
    uint112 private _reserve1;

    constructor(address _token0, address _token1, uint112 reserve0_, uint112 reserve1_) {
        // Enforce Uniswap V2 token ordering: token0 < token1
        if (_token0 < _token1) {
            token0 = _token0;
            token1 = _token1;
            _reserve0 = reserve0_;
            _reserve1 = reserve1_;
        } else {
            token0 = _token1;
            token1 = _token0;
            _reserve0 = reserve1_;
            _reserve1 = reserve0_;
        }
    }

    function getReserves()
        external
        view
        returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
    {
        return (_reserve0, _reserve1, uint32(block.timestamp));
    }

    function setReserves(uint112 reserve0_, uint112 reserve1_) external {
        _reserve0 = reserve0_;
        _reserve1 = reserve1_;
    }
}
