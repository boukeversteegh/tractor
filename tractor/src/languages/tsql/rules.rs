//! Per-kind transformation rules for T-SQL: the `TsqlKind` → `Rule` table.
//!
//! Read this file to find the rule for a specific kind. Read
//! [`super::transformations`] for the body of any `Rule::Custom`
//! handler. Read [`super::semantic`] for the output vocabulary.
//!
//! Exhaustive over `TsqlKind` — the compiler enforces coverage.
//!
//! T-SQL is the largest grammar in the project (552 kinds). The bulk
//! of those (367) are `Keyword*` reserved words that the dispatcher
//! detaches uniformly via `Rule::Detach` — their text is already
//! carried by the surrounding source.

use crate::languages::rule::Rule;

use super::input::TsqlKind;
use super::output::*;
use super::transformations;

#[allow(clippy::too_many_lines)]
pub fn rule(k: TsqlKind) -> Rule {
    use Rule::*;
    match k {
        // ---- ExtractOpThenRename ---------------------------------------
        TsqlKind::Assignment       => ExtractOpThenRename(ASSIGN),
        TsqlKind::BinaryExpression => ExtractOpThenRename(COMPARE),

        // ---- Pure Flatten ----------------------------------------------
        TsqlKind::Term
        | TsqlKind::SelectExpression => Flatten { distribute_field: None },

        // ---- Custom (language-specific logic in transformations.rs) ---
        TsqlKind::Identifier      => Custom(transformations::identifier),
        TsqlKind::UnaryExpression => Custom(transformations::unary_expression),

        // ---- Pure Rename -----------------------------------------------
        TsqlKind::AddColumn          => Rename(ADD_COLUMN),
        TsqlKind::AllFields          => Rename(STAR),
        TsqlKind::AlterTable         => Rename(ALTER_TABLE),
        TsqlKind::BetweenExpression  => Rename(BETWEEN),
        TsqlKind::Case               => Rename(CASE),
        TsqlKind::Cast               => Rename(CAST),
        TsqlKind::Column             => Rename(COL),
        TsqlKind::ColumnDefinition   => Rename(DEFINITION),
        TsqlKind::ColumnDefinitions  => Rename(COLUMNS),
        TsqlKind::CreateFunction     => Rename(FUNCTION),
        TsqlKind::CreateIndex        => Rename(CREATE_INDEX),
        TsqlKind::CreateTable        => Rename(CREATE),
        TsqlKind::Cte                => Rename(CTE),
        TsqlKind::Delete             => Rename(DELETE),
        TsqlKind::Direction          => Rename(DIRECTION),
        TsqlKind::ExecuteStatement   => Rename(EXEC),
        TsqlKind::Exists             => Rename(EXISTS),
        TsqlKind::Field              => Rename(COLUMN),
        TsqlKind::From               => Rename(FROM),
        TsqlKind::FunctionArgument   => Rename(ARG),
        TsqlKind::FunctionArguments  => Rename(ARG),
        TsqlKind::FunctionBody       => Rename(BODY),
        TsqlKind::GoStatement        => Rename(GO),
        TsqlKind::GroupBy            => Rename(GROUP),
        TsqlKind::Having             => Rename(HAVING),
        TsqlKind::IndexFields        => Rename(INDEX_FIELDS),
        TsqlKind::Insert             => Rename(INSERT),
        TsqlKind::Int                => Rename(INT),
        TsqlKind::Invocation         => Rename(CALL),
        TsqlKind::Join               => Rename(JOIN),
        TsqlKind::List               => Rename(LIST),
        TsqlKind::Literal            => Rename(LITERAL),
        TsqlKind::Nvarchar           => Rename(NVARCHAR),
        TsqlKind::ObjectReference    => Rename(REF),
        TsqlKind::OrderBy            => Rename(ORDER),
        TsqlKind::OrderTarget        => Rename(TARGET),
        TsqlKind::PartitionBy        => Rename(PARTITION),
        TsqlKind::Program            => Rename(FILE),
        TsqlKind::Relation           => Rename(RELATION),
        TsqlKind::Select             => Rename(SELECT),
        TsqlKind::SetOperation       => Rename(UNION),
        TsqlKind::SetStatement       => Rename(SET),
        TsqlKind::Statement          => Rename(STATEMENT),
        TsqlKind::Subquery           => Rename(SUBQUERY),
        TsqlKind::Transaction        => Rename(TRANSACTION),
        TsqlKind::Update             => Rename(UPDATE),
        TsqlKind::Varchar            => Rename(VARCHAR),
        TsqlKind::WhenClause         => Rename(WHEN),
        TsqlKind::Where              => Rename(WHERE),
        TsqlKind::WindowFunction     => Rename(WINDOW),
        TsqlKind::WindowSpecification => Rename(OVER),

        // ---- Detach — keyword leaves and the `#` unary operator. The
        //      surrounding source text already carries these tokens, so
        //      the wrapper element adds no semantic.
        TsqlKind::OpUnaryOther
        | TsqlKind::KeywordAction
        | TsqlKind::KeywordAdd
        | TsqlKind::KeywordAdmin
        | TsqlKind::KeywordAfter
        | TsqlKind::KeywordAll
        | TsqlKind::KeywordAlter
        | TsqlKind::KeywordAlways
        | TsqlKind::KeywordAnalyze
        | TsqlKind::KeywordAnd
        | TsqlKind::KeywordAny
        | TsqlKind::KeywordArray
        | TsqlKind::KeywordAs
        | TsqlKind::KeywordAsc
        | TsqlKind::KeywordAtomic
        | TsqlKind::KeywordAttribute
        | TsqlKind::KeywordAuthorization
        | TsqlKind::KeywordAutoIncrement
        | TsqlKind::KeywordAvro
        | TsqlKind::KeywordBefore
        | TsqlKind::KeywordBegin
        | TsqlKind::KeywordBetween
        | TsqlKind::KeywordBigint
        | TsqlKind::KeywordBigserial
        | TsqlKind::KeywordBinPack
        | TsqlKind::KeywordBinary
        | TsqlKind::KeywordBit
        | TsqlKind::KeywordBoolean
        | TsqlKind::KeywordBox2d
        | TsqlKind::KeywordBox3d
        | TsqlKind::KeywordBrin
        | TsqlKind::KeywordBtree
        | TsqlKind::KeywordBy
        | TsqlKind::KeywordBytea
        | TsqlKind::KeywordCache
        | TsqlKind::KeywordCached
        | TsqlKind::KeywordCalled
        | TsqlKind::KeywordCascade
        | TsqlKind::KeywordCascaded
        | TsqlKind::KeywordCase
        | TsqlKind::KeywordCast
        | TsqlKind::KeywordChange
        | TsqlKind::KeywordChar
        | TsqlKind::KeywordCharacter
        | TsqlKind::KeywordCharacteristics
        | TsqlKind::KeywordCheck
        | TsqlKind::KeywordCollate
        | TsqlKind::KeywordColumn
        | TsqlKind::KeywordColumns
        | TsqlKind::KeywordComment
        | TsqlKind::KeywordCommit
        | TsqlKind::KeywordCommitted
        | TsqlKind::KeywordCompression
        | TsqlKind::KeywordCompute
        | TsqlKind::KeywordConcurrently
        | TsqlKind::KeywordConflict
        | TsqlKind::KeywordConnection
        | TsqlKind::KeywordConstraint
        | TsqlKind::KeywordConstraints
        | TsqlKind::KeywordCopy
        | TsqlKind::KeywordCost
        | TsqlKind::KeywordCreate
        | TsqlKind::KeywordCross
        | TsqlKind::KeywordCsv
        | TsqlKind::KeywordCurrent
        | TsqlKind::KeywordCurrentRole
        | TsqlKind::KeywordCurrentTimestamp
        | TsqlKind::KeywordCurrentUser
        | TsqlKind::KeywordCycle
        | TsqlKind::KeywordData
        | TsqlKind::KeywordDatabase
        | TsqlKind::KeywordDate
        | TsqlKind::KeywordDatetime
        | TsqlKind::KeywordDatetime2
        | TsqlKind::KeywordDatetimeoffset
        | TsqlKind::KeywordDecimal
        | TsqlKind::KeywordDeclare
        | TsqlKind::KeywordDefault
        | TsqlKind::KeywordDeferrable
        | TsqlKind::KeywordDeferred
        | TsqlKind::KeywordDefiner
        | TsqlKind::KeywordDelayed
        | TsqlKind::KeywordDelete
        | TsqlKind::KeywordDelimited
        | TsqlKind::KeywordDelimiter
        | TsqlKind::KeywordDesc
        | TsqlKind::KeywordDisable
        | TsqlKind::KeywordDistinct
        | TsqlKind::KeywordDo
        | TsqlKind::KeywordDouble
        | TsqlKind::KeywordDrop
        | TsqlKind::KeywordDuplicate
        | TsqlKind::KeywordEach
        | TsqlKind::KeywordElse
        | TsqlKind::KeywordEnable
        | TsqlKind::KeywordEncoding
        | TsqlKind::KeywordEncrypted
        | TsqlKind::KeywordEnd
        | TsqlKind::KeywordEngine
        | TsqlKind::KeywordEnum
        | TsqlKind::KeywordEscape
        | TsqlKind::KeywordEscaped
        | TsqlKind::KeywordExcept
        | TsqlKind::KeywordExclude
        | TsqlKind::KeywordExec
        | TsqlKind::KeywordExecute
        | TsqlKind::KeywordExists
        | TsqlKind::KeywordExplain
        | TsqlKind::KeywordExtended
        | TsqlKind::KeywordExtension
        | TsqlKind::KeywordExternal
        | TsqlKind::KeywordFalse
        | TsqlKind::KeywordFields
        | TsqlKind::KeywordFilter
        | TsqlKind::KeywordFirst
        | TsqlKind::KeywordFloat
        | TsqlKind::KeywordFollowing
        | TsqlKind::KeywordFollows
        | TsqlKind::KeywordFor
        | TsqlKind::KeywordForce
        | TsqlKind::KeywordForceNotNull
        | TsqlKind::KeywordForceNull
        | TsqlKind::KeywordForceQuote
        | TsqlKind::KeywordForeign
        | TsqlKind::KeywordFormat
        | TsqlKind::KeywordFreeze
        | TsqlKind::KeywordFrom
        | TsqlKind::KeywordFull
        | TsqlKind::KeywordFunction
        | TsqlKind::KeywordGenerated
        | TsqlKind::KeywordGeography
        | TsqlKind::KeywordGeometry
        | TsqlKind::KeywordGin
        | TsqlKind::KeywordGist
        | TsqlKind::KeywordGroup
        | TsqlKind::KeywordGroups
        | TsqlKind::KeywordHash
        | TsqlKind::KeywordHaving
        | TsqlKind::KeywordHeader
        | TsqlKind::KeywordHighPriority
        | TsqlKind::KeywordIf
        | TsqlKind::KeywordIgnore
        | TsqlKind::KeywordImage
        | TsqlKind::KeywordImmediate
        | TsqlKind::KeywordImmutable
        | TsqlKind::KeywordIn
        | TsqlKind::KeywordInclude
        | TsqlKind::KeywordIncrement
        | TsqlKind::KeywordIncremental
        | TsqlKind::KeywordIndex
        | TsqlKind::KeywordInet
        | TsqlKind::KeywordInitially
        | TsqlKind::KeywordInner
        | TsqlKind::KeywordInout
        | TsqlKind::KeywordInput
        | TsqlKind::KeywordInsert
        | TsqlKind::KeywordInstead
        | TsqlKind::KeywordInt
        | TsqlKind::KeywordIntersect
        | TsqlKind::KeywordInterval
        | TsqlKind::KeywordInto
        | TsqlKind::KeywordInvoker
        | TsqlKind::KeywordIs
        | TsqlKind::KeywordIsolation
        | TsqlKind::KeywordJoin
        | TsqlKind::KeywordJson
        | TsqlKind::KeywordJsonb
        | TsqlKind::KeywordJsonfile
        | TsqlKind::KeywordKey
        | TsqlKind::KeywordLanguage
        | TsqlKind::KeywordLast
        | TsqlKind::KeywordLateral
        | TsqlKind::KeywordLeakproof
        | TsqlKind::KeywordLeft
        | TsqlKind::KeywordLevel
        | TsqlKind::KeywordLike
        | TsqlKind::KeywordLimit
        | TsqlKind::KeywordLines
        | TsqlKind::KeywordLocal
        | TsqlKind::KeywordLocation
        | TsqlKind::KeywordLogged
        | TsqlKind::KeywordLowPriority
        | TsqlKind::KeywordMain
        | TsqlKind::KeywordMatch
        | TsqlKind::KeywordMatched
        | TsqlKind::KeywordMaterialized
        | TsqlKind::KeywordMaxvalue
        | TsqlKind::KeywordMediumint
        | TsqlKind::KeywordMerge
        | TsqlKind::KeywordMetadata
        | TsqlKind::KeywordMinvalue
        | TsqlKind::KeywordModify
        | TsqlKind::KeywordMoney
        | TsqlKind::KeywordName
        | TsqlKind::KeywordNames
        | TsqlKind::KeywordNatural
        | TsqlKind::KeywordNchar
        | TsqlKind::KeywordNew
        | TsqlKind::KeywordNo
        | TsqlKind::KeywordNone
        | TsqlKind::KeywordNoscan
        | TsqlKind::KeywordNot
        | TsqlKind::KeywordNothing
        | TsqlKind::KeywordNowait
        | TsqlKind::KeywordNull
        | TsqlKind::KeywordNulls
        | TsqlKind::KeywordNumeric
        | TsqlKind::KeywordNvarchar
        | TsqlKind::KeywordObjectId
        | TsqlKind::KeywordOf
        | TsqlKind::KeywordOff
        | TsqlKind::KeywordOffset
        | TsqlKind::KeywordOid
        | TsqlKind::KeywordOids
        | TsqlKind::KeywordOld
        | TsqlKind::KeywordOn
        | TsqlKind::KeywordOnly
        | TsqlKind::KeywordOptimize
        | TsqlKind::KeywordOption
        | TsqlKind::KeywordOr
        | TsqlKind::KeywordOrc
        | TsqlKind::KeywordOrder
        | TsqlKind::KeywordOrdinality
        | TsqlKind::KeywordOthers
        | TsqlKind::KeywordOut
        | TsqlKind::KeywordOuter
        | TsqlKind::KeywordOver
        | TsqlKind::KeywordOverwrite
        | TsqlKind::KeywordOwned
        | TsqlKind::KeywordOwner
        | TsqlKind::KeywordParallel
        | TsqlKind::KeywordParquet
        | TsqlKind::KeywordPartition
        | TsqlKind::KeywordPartitioned
        | TsqlKind::KeywordPassword
        | TsqlKind::KeywordPermissive
        | TsqlKind::KeywordPlain
        | TsqlKind::KeywordPolicy
        | TsqlKind::KeywordPrecedes
        | TsqlKind::KeywordPreceding
        | TsqlKind::KeywordPrecision
        | TsqlKind::KeywordPrimary
        | TsqlKind::KeywordProcedure
        | TsqlKind::KeywordProgram
        | TsqlKind::KeywordPublic
        | TsqlKind::KeywordQuote
        | TsqlKind::KeywordRange
        | TsqlKind::KeywordRcfile
        | TsqlKind::KeywordRead
        | TsqlKind::KeywordReal
        | TsqlKind::KeywordRecursive
        | TsqlKind::KeywordReferences
        | TsqlKind::KeywordReferencing
        | TsqlKind::KeywordRegclass
        | TsqlKind::KeywordRegnamespace
        | TsqlKind::KeywordRegproc
        | TsqlKind::KeywordRegtype
        | TsqlKind::KeywordRename
        | TsqlKind::KeywordRepeatable
        | TsqlKind::KeywordReplace
        | TsqlKind::KeywordReplication
        | TsqlKind::KeywordReset
        | TsqlKind::KeywordRestart
        | TsqlKind::KeywordRestrict
        | TsqlKind::KeywordRestricted
        | TsqlKind::KeywordRestrictive
        | TsqlKind::KeywordReturn
        | TsqlKind::KeywordReturning
        | TsqlKind::KeywordReturns
        | TsqlKind::KeywordRewrite
        | TsqlKind::KeywordRight
        | TsqlKind::KeywordRole
        | TsqlKind::KeywordRollback
        | TsqlKind::KeywordRow
        | TsqlKind::KeywordRows
        | TsqlKind::KeywordSafe
        | TsqlKind::KeywordSchema
        | TsqlKind::KeywordSecurity
        | TsqlKind::KeywordSelect
        | TsqlKind::KeywordSeparator
        | TsqlKind::KeywordSequence
        | TsqlKind::KeywordSequencefile
        | TsqlKind::KeywordSerial
        | TsqlKind::KeywordSerializable
        | TsqlKind::KeywordSession
        | TsqlKind::KeywordSessionUser
        | TsqlKind::KeywordSet
        | TsqlKind::KeywordSetof
        | TsqlKind::KeywordShow
        | TsqlKind::KeywordSimilar
        | TsqlKind::KeywordSmalldatetime
        | TsqlKind::KeywordSmallint
        | TsqlKind::KeywordSmallmoney
        | TsqlKind::KeywordSmallserial
        | TsqlKind::KeywordSnapshot
        | TsqlKind::KeywordSome
        | TsqlKind::KeywordSort
        | TsqlKind::KeywordSpgist
        | TsqlKind::KeywordSplit
        | TsqlKind::KeywordStable
        | TsqlKind::KeywordStart
        | TsqlKind::KeywordStatement
        | TsqlKind::KeywordStatistics
        | TsqlKind::KeywordStats
        | TsqlKind::KeywordStdin
        | TsqlKind::KeywordStorage
        | TsqlKind::KeywordStored
        | TsqlKind::KeywordStrict
        | TsqlKind::KeywordString
        | TsqlKind::KeywordSupport
        | TsqlKind::KeywordTable
        | TsqlKind::KeywordTables
        | TsqlKind::KeywordTablespace
        | TsqlKind::KeywordTablets
        | TsqlKind::KeywordTblproperties
        | TsqlKind::KeywordTemp
        | TsqlKind::KeywordTemporary
        | TsqlKind::KeywordTerminated
        | TsqlKind::KeywordText
        | TsqlKind::KeywordTextfile
        | TsqlKind::KeywordThen
        | TsqlKind::KeywordTies
        | TsqlKind::KeywordTime
        | TsqlKind::KeywordTimestamp
        | TsqlKind::KeywordTimestamptz
        | TsqlKind::KeywordTinyint
        | TsqlKind::KeywordTo
        | TsqlKind::KeywordTransaction
        | TsqlKind::KeywordTrigger
        | TsqlKind::KeywordTrue
        | TsqlKind::KeywordTruncate
        | TsqlKind::KeywordType
        | TsqlKind::KeywordUnbounded
        | TsqlKind::KeywordUncached
        | TsqlKind::KeywordUncommitted
        | TsqlKind::KeywordUnion
        | TsqlKind::KeywordUnique
        | TsqlKind::KeywordUnload
        | TsqlKind::KeywordUnlogged
        | TsqlKind::KeywordUnsafe
        | TsqlKind::KeywordUnsigned
        | TsqlKind::KeywordUntil
        | TsqlKind::KeywordUpdate
        | TsqlKind::KeywordUse
        | TsqlKind::KeywordUser
        | TsqlKind::KeywordUsing
        | TsqlKind::KeywordUuid
        | TsqlKind::KeywordVacuum
        | TsqlKind::KeywordValid
        | TsqlKind::KeywordValue
        | TsqlKind::KeywordValues
        | TsqlKind::KeywordVarbinary
        | TsqlKind::KeywordVarchar
        | TsqlKind::KeywordVariadic
        | TsqlKind::KeywordVarying
        | TsqlKind::KeywordVerbose
        | TsqlKind::KeywordVersion
        | TsqlKind::KeywordView
        | TsqlKind::KeywordVirtual
        | TsqlKind::KeywordVolatile
        | TsqlKind::KeywordWait
        | TsqlKind::KeywordWhen
        | TsqlKind::KeywordWhere
        | TsqlKind::KeywordWhile
        | TsqlKind::KeywordWindow
        | TsqlKind::KeywordWith
        | TsqlKind::KeywordWithout
        | TsqlKind::KeywordWrite
        | TsqlKind::KeywordXml
        | TsqlKind::KeywordZerofill
        | TsqlKind::KeywordZone => Detach,

        // ---- Passthrough — kind name already matches the vocabulary,
        //      OR the kind is unhandled and survives as raw kind name.

        // Already matches our vocabulary.
        TsqlKind::Comment => Custom(transformations::passthrough),

        // ---- Unhandled in the previous dispatcher — survive as raw
        //      kind names. Most are TODO candidates for real semantics.

        // TODO: alter_* extras (Database/Function/Index/Policy/Procedure/
        // Role/Schema/Sequence/Type/View). Sibling of `alter_table` →
        // ALTER_TABLE. Each could rename to a per-target ALTER_X constant
        // or share a marker.
        TsqlKind::AlterColumn
        | TsqlKind::AlterDatabase
        | TsqlKind::AlterFunction
        | TsqlKind::AlterIndex
        | TsqlKind::AlterPolicy
        | TsqlKind::AlterProcedure
        | TsqlKind::AlterRole
        | TsqlKind::AlterSchema
        | TsqlKind::AlterSequence
        | TsqlKind::AlterType
        | TsqlKind::AlterView => Custom(transformations::passthrough),

        // TODO: create_* extras. Sibling of `create_table` → CREATE.
        TsqlKind::CreateDatabase
        | TsqlKind::CreateExtension
        | TsqlKind::CreateMaterializedView
        | TsqlKind::CreatePolicy
        | TsqlKind::CreateProcedure
        | TsqlKind::CreateQuery
        | TsqlKind::CreateRole
        | TsqlKind::CreateSchema
        | TsqlKind::CreateSequence
        | TsqlKind::CreateTrigger
        | TsqlKind::CreateType
        | TsqlKind::CreateView => Custom(transformations::passthrough),

        // TODO: drop_* statements. Each could rename to DROP with a
        // per-target marker.
        TsqlKind::DropColumn
        | TsqlKind::DropConstraint
        | TsqlKind::DropDatabase
        | TsqlKind::DropExtension
        | TsqlKind::DropFunction
        | TsqlKind::DropIndex
        | TsqlKind::DropRole
        | TsqlKind::DropSchema
        | TsqlKind::DropSequence
        | TsqlKind::DropTable
        | TsqlKind::DropType
        | TsqlKind::DropView => Custom(transformations::passthrough),

        // TODO: data-type kinds that aren't yet renamed. Each is the
        // grammar's leaf for its corresponding SQL type and could
        // rename to a TYPE marker.
        TsqlKind::Bigint
        | TsqlKind::Binary
        | TsqlKind::Bit
        | TsqlKind::Char
        | TsqlKind::Datetimeoffset
        | TsqlKind::Decimal
        | TsqlKind::Double
        | TsqlKind::Float
        | TsqlKind::Interval
        | TsqlKind::Mediumint
        | TsqlKind::Nchar
        | TsqlKind::Numeric
        | TsqlKind::Smallint
        | TsqlKind::Time
        | TsqlKind::Timestamp
        | TsqlKind::Tinyint
        | TsqlKind::Varbinary => Custom(transformations::passthrough),

        // TODO: function-attribute clauses. Specific to PostgreSQL
        // function declarations; currently all passthrough.
        TsqlKind::FunctionCost
        | TsqlKind::FunctionDeclaration
        | TsqlKind::FunctionLanguage
        | TsqlKind::FunctionLeakproof
        | TsqlKind::FunctionRows
        | TsqlKind::FunctionSafety
        | TsqlKind::FunctionSecurity
        | TsqlKind::FunctionStrictness
        | TsqlKind::FunctionSupport
        | TsqlKind::FunctionVolatility => Custom(transformations::passthrough),

        // TODO: comparison and pattern operators (LIKE/SIMILAR/IN/NOT
        // forms, DISTINCT FROM). Currently passthrough.
        TsqlKind::DistinctFrom
        | TsqlKind::IsNot
        | TsqlKind::NotDistinctFrom
        | TsqlKind::NotIn
        | TsqlKind::NotLike
        | TsqlKind::NotSimilarTo
        | TsqlKind::SimilarTo => Custom(transformations::passthrough),

        // TODO: control / DDL / misc.
        TsqlKind::AddConstraint
        | TsqlKind::Array
        | TsqlKind::ArraySizeDefinition
        | TsqlKind::AssignmentList
        | TsqlKind::Bang
        | TsqlKind::Block
        | TsqlKind::ChangeColumn
        | TsqlKind::ChangeOwnership
        | TsqlKind::ColumnPosition
        | TsqlKind::CommentStatement
        | TsqlKind::CompositeField
        | TsqlKind::Constraint
        | TsqlKind::Constraints
        | TsqlKind::CoveringColumns
        | TsqlKind::CrossJoin
        | TsqlKind::DollarQuote
        | TsqlKind::Enum
        | TsqlKind::EnumElements
        | TsqlKind::FilterExpression
        | TsqlKind::FrameDefinition
        | TsqlKind::IndexHint
        | TsqlKind::LateralCrossJoin
        | TsqlKind::LateralJoin
        | TsqlKind::Limit
        | TsqlKind::Marginalia
        | TsqlKind::ModifyColumn
        | TsqlKind::ObjectId
        | TsqlKind::Offset
        | TsqlKind::OpOther
        | TsqlKind::OrderedColumns
        | TsqlKind::Parameter
        | TsqlKind::ParenthesizedExpression
        | TsqlKind::RenameColumn
        | TsqlKind::RenameObject
        | TsqlKind::ResetStatement
        | TsqlKind::Returning
        | TsqlKind::RowFormat
        | TsqlKind::SetConfiguration
        | TsqlKind::SetSchema
        | TsqlKind::StorageLocation
        | TsqlKind::StorageParameters
        | TsqlKind::StoredAs
        | TsqlKind::Subscript
        | TsqlKind::TableOption
        | TsqlKind::TablePartition
        | TsqlKind::TableSort
        | TsqlKind::Tablespace
        | TsqlKind::TabletSplit
        | TsqlKind::Values
        | TsqlKind::VarDeclaration
        | TsqlKind::VarDeclarations
        | TsqlKind::WhileStatement
        | TsqlKind::WindowClause
        | TsqlKind::WindowFrame => Custom(transformations::passthrough),
    }
}
