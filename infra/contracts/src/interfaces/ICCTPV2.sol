// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Minimal subset of Circle's CCTP V2 contracts used by RebalanceExecutor.
interface ICCTPV2MessageTransmitter {
    /// @dev Called by anyone with the Circle-attested message + signature; the
    /// transmitter forwards the body to the encoded `destinationCaller` (our
    /// RebalanceExecutor) via a downstream `handleReceiveMessage`.
    function receiveMessage(bytes calldata message, bytes calldata attestation)
        external
        returns (bool success);
}

/// @notice Implemented by destination-chain hook recipients.
interface IMessageHandlerV2 {
    /// @param sourceDomain The CCTP V2 source domain that initiated the burn.
    /// @param sender The 32-byte representation of the burner address.
    /// @param messageBody Hook data set by the source-chain caller. Aegis
    ///        encodes `(address recipient, address tokenOut, uint24 fee,
    ///        uint256 minOut, uint256 deadline)` here.
    function handleReceiveMessage(uint32 sourceDomain, bytes32 sender, bytes calldata messageBody)
        external
        returns (bool success);
}
