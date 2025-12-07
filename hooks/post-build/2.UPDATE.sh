#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

require_command "cz" || echo "pls install python-commitizen"

cz ch # 更新release日志

if [ "$KAM_BUMP_ENABLED" = "1" ]; then
    cz bump && kam version patch || echo "Nothing to bump." # 更新版本号
    exit 0
fi
