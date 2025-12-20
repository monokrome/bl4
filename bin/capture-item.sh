#!/bin/bash
# Capture item screenshots for research

ITEMS_DIR="share/items"
STAGING_DIR="$ITEMS_DIR/.staging"

mkdir -p "$STAGING_DIR"

finalize_item() {
    local uuid="$1"
    local staging="$STAGING_DIR/$uuid"
    local dest="$ITEMS_DIR/$uuid"

    if [[ -d "$staging" ]] && [[ -n "$(ls -A "$staging" 2>/dev/null)" ]]; then
        mv "$staging" "$dest"
        echo "    Finalized: $uuid"
    else
        rm -rf "$staging" 2>/dev/null
    fi
}

start_item() {
    local uuid="$1"
    local dir="$STAGING_DIR/$uuid"
    mkdir -p "$dir"

    # Find next screenshot number
    local count=1
    while [[ -f "$dir/$(printf '%02d' $count).png" ]]; do
        ((count++))
    done

    echo ""
    echo "Item: $uuid (staging)"
    echo ""
    echo "ENTER=screenshot, n=finalize & new, q=finalize & quit"
    echo ""

    while true; do
        read -r -p "[$count] > " input

        if [[ "$input" == "q" || "$input" == "Q" ]]; then
            finalize_item "$uuid"
            echo "Done."
            exit 0
        fi

        if [[ "$input" == "n" || "$input" == "N" ]]; then
            finalize_item "$uuid"
            start_item "$(uuidgen)"
            return
        fi

        filename="$dir/$(printf '%02d' $count).png"
        if grim -g "$(slurp)" "$filename" 2>/dev/null; then
            echo "    Saved: $filename"
            ((count++))
        else
            echo "    (cancelled)"
        fi
    done
}

# Use provided UUID or generate new one
if [[ -n "$1" ]]; then
    start_item "$1"
else
    start_item "$(uuidgen)"
fi
