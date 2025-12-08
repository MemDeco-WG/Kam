#! bin/sh

# Hello Kam

# This is a customize script for the kam module
# You can add custom installation or configuration commands here

# "Installing Kam module..."

# ui_print <msg>
#     print <msg> to console
#     Avoid using 'echo' as it will not display in custom recovery's console

# abort <msg>
#     print error message <msg> to console and terminate the installation
#     Avoid using 'exit' as it will skip the termination cleanup steps

# set_perm <target> <owner> <group> <permission> [context]
#     if [context] is not set, the default is "u:object_r:system_file:s0"
#     this function is a shorthand for the following commands:
#        chown owner.group target
#        chmod permission target
#        chcon context target

# set_perm_recursive <directory> <owner> <group> <dirpermission> <filepermission> [context]
#     if [context] is not set, the default is "u:object_r:system_file:s0"
#     for all files in <directory>, it will call:
#        set_perm file owner group filepermission context
#     for all directories in <directory> (including itself), it will call:
#        set_perm dir owner group dirpermission context

# KSU (bool): a variable to mark that the script is running in the KernelSU environment, and the value of this variable will always be true. You can use it to distinguish between KernelSU and Magisk.
# KSU_VER (string): the version string of currently installed KernelSU (e.g. v0.4.0).
# KSU_VER_CODE (int): the version code of currently installed KernelSU in userspace (e.g. 10672).
# KSU_KERNEL_VER_CODE (int): the version code of currently installed KernelSU in kernel space (e.g. 10672).
# BOOTMODE (bool): always be true in KernelSU.
# MODPATH (path): the path where your module files should be installed.
# TMPDIR (path): a place where you can temporarily store files.
# ZIPFILE (path): your module's installation ZIP.
# ARCH (string): the CPU architecture of the device. Value is either arm, arm64, x86, or x64.
# IS64BIT (bool): true if $ARCH is either arm64 or x64.
# API (int): the API level (Android version) of the device (e.g., 23 for Android 6.0).

ui_print "Installing Kam module..."
