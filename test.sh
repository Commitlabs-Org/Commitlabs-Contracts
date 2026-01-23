#!/bin/bash
# Simple test script for running all tests

echo "🧪 Running all tests..."
cargo test --workspace --release

if [ $? -eq 0 ]; then
    echo "✅ All tests passed!"
else
    echo "❌ Some tests failed"
    exit 1
fi
