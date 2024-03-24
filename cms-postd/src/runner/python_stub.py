# -*- coding: utf-8 -*-

def CMSException(httpStatusCode=500, message=''):
    raise Exception(f'{httpStatusCode}: {message}')

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
            raise CMSException(400, 'Form data is too long.')
        if charset is not None and [ c for c in field if c not in charset ]:
            raise CMSException(400, 'Invalid character in form data')
        return field

    def getBool(self, name, default=False, maxlen=32, charset=defaultCharsetBool):
        field = self.getStr(name, '', maxlen, charset)
        if not field:
            return default
        return stringBool(field, default)

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

try:
    # Import the post handler module file.
    import importlib
    loader = importlib.machinery.SourceFileLoader(handler_mod_name, handler_mod_path)
    module = loader.load_module()

    module.CMSException = CMSException
    module.CMSPostException = CMSException

    # Get the post() handler function from the module.
    post_handler = getattr(module, 'post', None)
    if post_handler is None:
        raise Exception('No post() handler function found in module.')
except Exception as e:
    raise Exception(f'Load post handler module [{type(e)}]: {e}')

try:
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
except Exception as e:
    raise Exception(f'Run post handler [{type(e)}]: {e}')

# vim: ts=4 sw=4 expandtab
