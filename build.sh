#!/bin/bash

# Exit on error
set -e

echo "🧹 Cleaning previous builds..."
rm -rf dist/ pkg/ 
#cd src-tauri && cargo clean
#cd ..

# Ensure target directory exists
#mkdir -p target

#echo "🎨 Generating icons..."
#cd src-tauri
#./iconmaker.sh
#cd ..

# Set the OUT_DIR environment variable explicitly
#export OUT_DIR="$(pwd)/target"

echo "🦀 Building WASM components..."
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web

echo "📦 Building frontend assets..."
npx webpack --mode production

#echo "🚀 Building and running Tauri application..."
#cargo tauri dev
