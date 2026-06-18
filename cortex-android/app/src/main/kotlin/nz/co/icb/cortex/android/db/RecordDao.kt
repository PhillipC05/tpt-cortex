package nz.co.icb.cortex.android.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query

@Dao
interface RecordDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(record: RecordEntity): Long

    @Query("SELECT * FROM records WHERE tableName = :tableName ORDER BY createdAt DESC")
    suspend fun queryByTable(tableName: String): List<RecordEntity>

    @Query("SELECT * FROM records ORDER BY createdAt DESC")
    suspend fun queryAll(): List<RecordEntity>

    @Query("DELETE FROM records")
    suspend fun deleteAll()

    @Query("DELETE FROM records WHERE tableName = :tableName")
    suspend fun deleteByTable(tableName: String)
}
