#!/bin/bash
# Mockup: What colored mermaid flowchart ASCII would look like
# Run: bash docs/superpowers/specs/2026-04-22-mermaid-color-mockup.sh
#
# Simulates this mermaid source:
#   graph TD
#     A[Start] --> B{Decision}
#     B -->|Yes| C[Action]
#     B -->|No| D[End]
#     style A fill:#ff9900,stroke:#cc3333,color:#ffffff
#     style B stroke:#336633
#     classDef result fill:#6699ff
#     class C,D result
#     linkStyle 0 stroke:#ff3333
#     linkStyle 1 stroke:#33cc33

# Clay theme nearest-match colors:
# #ff9900 → Dark Honey   (210,140,40)
# #cc3333 → Clay Red     (180,90,60)
# #ffffff → Body          (190,180,160)
# #336633 → Olive         (120,160,80)
# #6699ff → diagram_border(160,120,60) [nearest warm match]
# #ff3333 → Clay Red     (180,90,60)
# #33cc33 → Olive         (120,160,80)

# Color escape codes (Clay theme RGB values)
HONEY="\033[38;2;210;140;40m"      # Dark Honey - node fill
CLAY_RED="\033[38;2;180;90;60m"    # Clay Red - node border / edge stroke
BODY="\033[38;2;190;180;160m"      # Body text - node text
OLIVE="\033[38;2;120;160;80m"      # Olive - edge stroke
DRIFTWOOD="\033[38;2;130;140;110m" # Driftwood - edge label
AMBER="\033[38;2;160;120;60m"      # Amber - node fill (nearest blue match)
RESET="\033[0m"

echo ""
echo "  Mermaid Flowchart with Color (Clay Theme)"
echo "  =========================================="
echo ""
echo "  Source:"
echo "    graph TD"
echo "      A[Start] --> B{Decision}"
echo "      B -->|Yes| C[Action]"
echo "      B -->|No| D[End]"
echo "      style A fill:#ff9900,stroke:#cc3333,color:#ffffff"
echo "      style B stroke:#336633"
echo "      classDef result fill:#6699ff"
echo "      class C,D result"
echo "      linkStyle 0 stroke:#ff3333"
echo "      linkStyle 1 stroke:#33cc33"
echo ""
echo "  Rendered:"
echo ""

# Node A: styled with fill=honey, stroke=clay_red, color=body
echo -e "          ${CLAY_RED}┌───────┐${RESET}"
echo -e "          ${CLAY_RED}│${HONEY} ${BODY}Start${HONEY} ${CLAY_RED}│${RESET}"
echo -e "          ${CLAY_RED}└───┬───┘${RESET}"

# Edge A→B: linkStyle 0 stroke=clay_red
echo -e "          ${CLAY_RED}    │${RESET}"
echo -e "          ${CLAY_RED}    ▼${RESET}"

# Node B: styled with stroke=olive (no fill/color, so those stay default)
echo -e "          ${OLIVE}┌───────────┐${RESET}"
echo -e "          ${OLIVE}│${RESET}  Decision  ${OLIVE}│${RESET}"
echo -e "          ${OLIVE}└─────┬─────┘${RESET}"

# Edges from B
echo -e "        ${OLIVE}┌───┘     └───┐${RESET}"
echo -e "        ${OLIVE}│${RESET} ${DRIFTWOOD}Yes${RESET}       ${DRIFTWOOD}No${RESET} ${OLIVE}│${RESET}"
echo -e "        ${OLIVE}▼${RESET}             ${OLIVE}▼${RESET}"

# Node C and D: classDef result fill=amber
echo -e "    ${AMBER}┌────────┐${RESET}     ${AMBER}┌─────┐${RESET}"
echo -e "    ${AMBER}│${RESET} Action ${AMBER}│${RESET}     ${AMBER}│${RESET} End ${AMBER}│${RESET}"
echo -e "    ${AMBER}└────────┘${RESET}     ${AMBER}└─────┘${RESET}"

echo ""
echo ""
echo "  Comparison: same diagram WITHOUT style directives (current behavior):"
echo ""

# Unstyled version
echo "          ┌───────┐"
echo "          │ Start │"
echo "          └───┬───┘"
echo "              │"
echo "              ▼"
echo "          ┌───────────┐"
echo "          │  Decision  │"
echo "          └─────┬─────┘"
echo "        ┌───┘     └───┐"
echo "        │ Yes       No │"
echo "        ▼             ▼"
echo "    ┌────────┐     ┌─────┐"
echo "    │ Action │     │ End │"
echo "    └────────┘     └─────┘"
echo ""
