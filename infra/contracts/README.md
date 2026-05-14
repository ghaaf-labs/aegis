# Aegis Contracts

Foundry workspace for Aegis's destination-chain CCTP V2 hook executor.

## Layout

```
src/
  RebalanceExecutor.sol            -- handleReceiveMessage hook target
  interfaces/
    ICCTPV2.sol                    -- minimal CCTP V2 receiver + transmitter
    IUniswapV3SwapRouter.sol       -- exactInputSingle subset
test/
  RebalanceExecutor.t.sol          -- unit tests against mock router + USDC
script/
  Deploy.s.sol                     -- foundry deploy script
HOOK_ABI.md                        -- hook payload spec (used by apps/api)
```

## Local test loop

```
forge test
forge fmt --check
```

## Deploy

```
export MESSAGE_TRANSMITTER=0xE737e5cEBEEBa77EFE34D4aa090756590b1CE275
export USDC=0x036CbD53842c5426634e7929541eC2318f3dCF7e
export SWAP_ROUTER=0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4
forge script script/Deploy.s.sol \
  --rpc-url base_sepolia \
  --account aegis-deployer \
  --sender 0xYOUR_DEPLOYER \
  --broadcast --verify
```

After a successful deploy, copy the printed `RebalanceExecutor` address into
`packages/shared/src/constants.ts` under `CHAIN_ADDRESSES.<chain>.rebalanceExecutor`.
