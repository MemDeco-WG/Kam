#!/system/bin/sh

# {{name}} Service Script
# Author: {{author}}
# Version: {{version}}

MODDIR=${0%/*}

# Your service code here
echo "Starting {{name}} service..."

# Example: Monitor battery level
while true; do
    battery_level=$(cat /sys/class/power_supply/battery/capacity 2>/dev/null || echo "unknown")
    echo "Battery level: $battery_level" >> $MODDIR/service.log
    sleep 60
done
