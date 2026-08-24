#include "stdafx.h"

DECLARE_COMPONENT_VERSION(
    "Omniphony Output",
    "0.1.0",
    "Single-render Omniphony frontend for foobar2000.\n"
    "The visible output is Output: Omniphony.\n"
    "Windows shared RAW is an internal transport detail.");

VALIDATE_COMPONENT_FILENAME("foo_out_omniphony.dll");

FOOBAR2000_IMPLEMENT_CFG_VAR_DOWNGRADE;

