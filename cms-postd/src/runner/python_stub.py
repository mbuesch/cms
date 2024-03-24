# -*- coding: utf-8 -*-

class CMSFormFields:
    UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
    LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'
    NUMBERS   = '0123456789'
    defaultCharset     = LOWERCASE + UPPERCASE + NUMBERS + '-_. \t'
    defaultCharsetBool = LOWERCASE + UPPERCASE + NUMBERS + ' \t'
    defaultCharsetInt  = NUMBERS + 'xXabcdefABCDEF- \t'

    def __init__(self, forms):
        assert isinstance(forms, dict)
        self.__forms = forms

    def getStr(self, name, default='', maxlen=32, charset=defaultCharset):
        field = self.__forms.get(name, default.encode('UTF-8', 'strict'))
        if field is None:
            return None
        assert isinstance(field, bytes)
        field = field.decode('UTF-8', 'strict')
        if maxlen is not None and len(field) > maxlen:
            raise self.CMSPostException('Form data is too long.')
        if charset is not None and [ c for c in field if c not in charset ]:
            raise self.CMSPostException('Invalid character in form data')
        return field

    def getBool(self, name, default=False, maxlen=32, charset=defaultCharsetBool):
        field = self.getStr(name, '', maxlen, charset)
        if not field:
            return default
        s = field.lower()
        if s in ("true", "yes", "on", "1"):
            return True
        if s in ("false", "no", "off", "0"):
            return False
        try:
            return bool(int(s))
        except ValueError:
            return default

    def getInt(self, name, default=0, maxlen=32, charset=defaultCharsetInt):
        field = self.getStr(name, '', maxlen, charset)
        if not field:
            return default
        try:
            field = field.lower().strip()
            if field.startswith('0x'):
                return int(field[2:], 16)
            return int(field)
        except ValueError:
            return default

# Import the post handler module file.
import importlib
import importlib.machinery
loader = importlib.machinery.SourceFileLoader(handler_mod_name, handler_mod_path)
module = loader.load_module()

module.CMSPostException = CMSPostException
CMSFormFields.CMSPostException = CMSPostException

# Get the post() handler function from the module.
post_handler = getattr(module, 'post', None)
if post_handler is None:
    raise CMSPostException('No post() handler function found in module.')

# Add post.py directory to include search path so that
# the post handler can import from it.
import sys
if handler_mod_dir not in sys.path:
    sys.path.insert(0, handler_mod_dir)

# Run the post handler.
reply_body, reply_mime = post_handler(
    CMSFormFields(request_form_fields),
    request_query,
)

# vim: ts=4 sw=4 expandtab
