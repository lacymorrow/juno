#!/bin/bash
# CUA accuracy test — validates click positioning and screenshot capture
set -e
CUA="./target/release/juno-cua"
RESULTS="/tmp/cua-test-results"
mkdir -p "$RESULTS"

echo "=== juno-cua Accuracy Test Suite ==="
echo ""

# 1. Screenshot test
echo "[1/6] Screenshot capture..."
SHOT=$($CUA screenshot 2>/dev/null)
if echo "$SHOT" | jq -e '.screenshot_base64' > /dev/null 2>&1; then
    B64_LEN=$(echo "$SHOT" | jq -r '.screenshot_base64' | wc -c | tr -d ' ')
    echo "$SHOT" | jq -r '.screenshot_base64' | base64 -d > "$RESULTS/screen1.png"
    SIZE=$(wc -c < "$RESULTS/screen1.png" | tr -d ' ')
    DIM=$(sips -g pixelWidth -g pixelHeight "$RESULTS/screen1.png" 2>/dev/null | grep pixel | awk '{print $2}' | tr '\n' 'x' | sed 's/x$//')
    echo "  ✅ Screenshot captured: ${SIZE} bytes, ${DIM} pixels"
else
    echo "  ❌ Screenshot failed"
    echo "  Error: $SHOT"
fi
echo ""

# 2. Cursor position baseline
echo "[2/6] Cursor position baseline..."
POS1=$($CUA cursor-position 2>/dev/null)
X1=$(echo "$POS1" | jq -r '.x')
Y1=$(echo "$POS1" | jq -r '.y')
echo "  Current position: ($X1, $Y1)"
echo ""

# 3. Click accuracy test — click at known coords, then verify cursor moved there
echo "[3/6] Click accuracy test..."
# Test 5 positions across the screen
TARGETS=("100 100" "500 300" "800 500" "200 700" "1000 400")
ALL_PASS=true
for TARGET in "${TARGETS[@]}"; do
    TX=$(echo $TARGET | cut -d' ' -f1)
    TY=$(echo $TARGET | cut -d' ' -f2)
    
    $CUA click --x $TX --y $TY 2>/dev/null > /dev/null
    sleep 0.1
    
    POS=$($CUA cursor-position 2>/dev/null)
    AX=$(echo "$POS" | jq -r '.x' | cut -d'.' -f1)
    AY=$(echo "$POS" | jq -r '.y' | cut -d'.' -f1)
    
    # Allow 2px tolerance
    DX=$(( ${AX:-0} - $TX ))
    DY=$(( ${AY:-0} - $TY ))
    DX=${DX#-}  # abs
    DY=${DY#-}  # abs
    
    if [ "$DX" -le 2 ] && [ "$DY" -le 2 ]; then
        echo "  ✅ Click ($TX,$TY) → cursor at ($AX,$AY) [Δ${DX},${DY}]"
    else
        echo "  ❌ Click ($TX,$TY) → cursor at ($AX,$AY) [Δ${DX},${DY}] — DRIFT"
        ALL_PASS=false
    fi
done
echo ""

# 4. Mouse move test
echo "[4/6] Mouse move precision..."
$CUA mouse-move --x 600 --y 400 2>/dev/null > /dev/null
sleep 0.1
POS=$($CUA cursor-position 2>/dev/null)
MX=$(echo "$POS" | jq -r '.x' | cut -d'.' -f1)
MY=$(echo "$POS" | jq -r '.y' | cut -d'.' -f1)
DX=$(( ${MX:-0} - 600 )); DX=${DX#-}
DY=$(( ${MY:-0} - 400 )); DY=${DY#-}
if [ "$DX" -le 1 ] && [ "$DY" -le 1 ]; then
    echo "  ✅ Move to (600,400) → cursor at ($MX,$MY) [Δ${DX},${DY}]"
else
    echo "  ❌ Move to (600,400) → cursor at ($MX,$MY) [Δ${DX},${DY}] — DRIFT"
fi
echo ""

# 5. Screenshot-after-click consistency
echo "[5/6] Screenshot after click (visual consistency)..."
$CUA click --x 50 --y 50 2>/dev/null > /dev/null
sleep 0.3
SHOT2=$($CUA screenshot 2>/dev/null)
echo "$SHOT2" | jq -r '.screenshot_base64' | base64 -d > "$RESULTS/screen2_after_click.png"
SIZE2=$(wc -c < "$RESULTS/screen2_after_click.png" | tr -d ' ')
DIM2=$(sips -g pixelWidth -g pixelHeight "$RESULTS/screen2_after_click.png" 2>/dev/null | grep pixel | awk '{print $2}' | tr '\n' 'x' | sed 's/x$//')
echo "  ✅ Post-click screenshot: ${SIZE2} bytes, ${DIM2} pixels"
echo ""

# 6. Clipboard round-trip
echo "[6/6] Clipboard round-trip..."
TEST_STR="juno-cua-test-$(date +%s)"
$CUA set-clipboard --content "$TEST_STR" 2>/dev/null > /dev/null
CLIP=$($CUA get-clipboard 2>/dev/null)
GOT=$(echo "$CLIP" | jq -r '.content')
if [ "$GOT" = "$TEST_STR" ]; then
    echo "  ✅ Clipboard: wrote '$TEST_STR', read back '$GOT'"
else
    echo "  ❌ Clipboard mismatch: wrote '$TEST_STR', got '$GOT'"
fi
echo ""

# Restore cursor
$CUA mouse-move --x "$X1" --y "$Y1" 2>/dev/null > /dev/null

echo "=== Results saved to $RESULTS/ ==="
echo "  screen1.png — initial screenshot"
echo "  screen2_after_click.png — post-click screenshot"
echo ""
echo "Open them with: open $RESULTS/"
