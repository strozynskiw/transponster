Additional assumptions:
- if client id from withdraw/dispute/resolve is different than the on in the referenced transaction, the transaction is ignored.
- lock account doesn't accept any operation
- when withdrawal is disputed, the disputed amount is added to held value. In this case total founds increases (while it remain the same when a deposit is disputed - as it suppose to according to the paper)