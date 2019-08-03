import sys
if sys.version_info[0] < 3 or sys.version_info[1] < 3:
	raise Exception("Need Python 3.3 or later")
del sys

from cms.cms import *
from cms.exception import *
