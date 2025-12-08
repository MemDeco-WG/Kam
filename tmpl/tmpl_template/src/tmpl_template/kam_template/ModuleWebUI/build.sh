#!/bin/bash
echo "BUILD FOR MODULE"
MODID="$1"
if [ -z "$MODID" ]; then
    echo "Usage: $0 <MODID>"
    exit 1
fi
if [ -d "node_modules" ]; then
    echo "node_modules exists"
else
    npm ci
fi
find src -name "*.js" -exec sed -i "s/ModuleWebUI/${MODID}/g" {} \;
sed -i "s/ModuleWebUI/${MODID}/g" index.html
npm run build:prod
echo "BUILD DONE"
