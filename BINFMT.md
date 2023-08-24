# Binary Format Specification

## Inode Table: 
> An array whose bounds are indicated in the file header, given in Offset (`u64`) and Length (`u64`) (number of entries, **not bytes**)

### Page Descriptor
> The Page Descriptor outline below is a description of the binary format which is parsed to yield a valid descriptor. The table rows must be parsed in order of appearance with no gaps.

|key|length/type|meaning|
|---|-----------|-------|
|Name|`u64`|An index into the string table|
|Access Control Entries|`u64`|The number of ACL entries which are defined on this page|
|[Access Control Entry]|(`u8` + `u64`) * _Access Control Entries_|A permission-hint byte specifying up to 8 unrelated permissions; An index into the string table|
|Chunks Entries|`u64`|The number of chunks the page uses to contain its data|
|[Chunks]|(`u64` + `u64`) * _Chunks Entries_|An Offset;Length in bytes pair specifying a range of data|

The remaining information the page descriptor includes is to be fetched from various other sources. Most of which can be found by consulting the journal (history table). 

This function writes the database header out-of-order. The logic behind this procedure is explained below
The database is defined using 5 structures;

1. Magic Header - This is a fixed-size array located at exactly offset 0. It provides the absolute outline of the database. 

    1. Magic Number (`u32`): used for sanity-checking the file. This number must be exactly 0x42445446 (Little-Endian notation), where anything else represents an error.

    2. Format Version (`u32`): used to instruct parsers which syntactical rules and patterns are permitted

    3. Reserved (`u64`): placeholder for future versions.

    4. INode Table Offset (`u64`): the byte offset (absolute) of the INode Table. Should be 0x10-aligned, although this is not strictly necessary.

    5. INode Table Length (`u64`): the number of items the inode table contains

    6. String Table Offset (`u64`): the byte offset (absolute) of the String Table. Should be 0x10-aligned, although this is not strictly necessary.

    7. String Table Length (`u64`): the number of items the string table contains

    8. History Table Offset (`u64`): the byte offset (absolute) of the History Table (Journal). Should be 0x10-aligned, although this is not strictly necessary.

    9. History Table Length (`u64`): the number of items the history table contains

    10. Meta Offset (`u64`): The byte offset (absolute) of the meta string

    11. Meta Length (`u64`): They byte length of the meta string

2. Meta     

### PageDescriptor

|key|length/type|meaning|
|---|-----------|-------|
|page_name|`u64`|Index in the string table used to identify the page. Should be unique - soft requirement|
|acl_len|`u64`|The number of ACL entries to parse|
|[acl]|(`u8` + `u64`) * _acl_len_|The Access Control objects to parse|
|_alignment_|%0x10|Align to the next 0x10th byte|
|inode_len|`u64`|The number of INode entries to parse|
|[inodes]|(`u64` + `u64`) * _inode_len_|The Inode entry (offset, len - bytes)|
