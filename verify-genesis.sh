#!/bin/bash
# Verification script for the genesis configuration
# This script verifies that all components of the genesis setup are present and valid

set -e

GENESIS_DIR="genesis-output"
echo "================================================"
echo "Genesis Configuration Verification"
echo "================================================"
echo ""

# Check if directory exists
if [ ! -d "$GENESIS_DIR" ]; then
    echo "❌ Error: Genesis directory not found"
    exit 1
fi

echo "✓ Genesis directory exists"

# Check for genesis files
if [ -f "$GENESIS_DIR/genesis.bin" ]; then
    echo "✓ genesis.bin found ($(du -h $GENESIS_DIR/genesis.bin | cut -f1))"
else
    echo "❌ genesis.bin missing"
    exit 1
fi

if [ -f "$GENESIS_DIR/genesis.tar.bz2" ]; then
    echo "✓ genesis.tar.bz2 found ($(du -h $GENESIS_DIR/genesis.tar.bz2 | cut -f1))"
else
    echo "❌ genesis.tar.bz2 missing"
    exit 1
fi

# Check for keypair files
for keypair in faucet.json identity.json vote-account.json stake-account.json; do
    if [ -f "$GENESIS_DIR/$keypair" ]; then
        echo "✓ $keypair exists"
    else
        echo "⚠ $keypair missing (may need to be regenerated)"
    fi
done

# Check for RocksDB directory
if [ -d "$GENESIS_DIR/rocksdb" ]; then
    echo "✓ rocksdb directory exists ($(du -sh $GENESIS_DIR/rocksdb | cut -f1))"
else
    echo "⚠ rocksdb directory missing (may need to be regenerated)"
fi

# Check for README
if [ -f "$GENESIS_DIR/README.md" ]; then
    echo "✓ README.md exists"
    echo ""
    echo "Genesis Hash: $(grep 'Genesis Hash' $GENESIS_DIR/README.md | head -1 | sed 's/.*: //')"
    echo "Creation Time: $(grep 'Creation Time' $GENESIS_DIR/README.md | head -1 | sed 's/.*: //')"
    echo "Cluster Type: $(grep 'Cluster Type' $GENESIS_DIR/README.md | head -1 | sed 's/.*: //')"
else
    echo "❌ README.md missing"
fi

echo ""
echo "================================================"
echo "Verification Complete"
echo "================================================"
