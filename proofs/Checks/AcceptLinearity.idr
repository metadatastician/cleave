module Checks.AcceptLinearity

import Cleave.Kernel.HandleLinearity

%default total

ok : (1 h : Handle) -> Perp
ok h = teardown h
