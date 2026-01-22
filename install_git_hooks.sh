#!/bin/sh
# Install git hooks for this repository

HOOKS_DIR=".git/hooks"

# Ensure we're in the repo root
if [ ! -d ".git" ]; then
    echo "Error: Must be run from the repository root"
    exit 1
fi

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Create pre-commit hook
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/sh
# Pre-commit hook: Run cargo fmt before committing

# Format all Rust files
cargo fmt

# Stage any formatting changes
git add -u

exit 0
EOF

# Make hook executable
chmod +x "$HOOKS_DIR/pre-commit"

echo "Git hooks installed successfully"
